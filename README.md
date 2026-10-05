# Pillbox

A browser-first multiplayer arena shooter where players build the arenas and rules. Three vertical station arenas, six visible weapons, Box3D capsule physics, lights and shadows, positional sound, and a browser sandbox. Rust runs the authoritative simulation and Bevy renders the WASM client.

> This repository does not own production infrastructure or hosting. It publishes a deployable container image; `crvouga/workspace` owns deployment.

## Run locally

Prerequisites: Rust 1.94 (the pinned toolchain installs the WASM target), Docker, Atlas, Node 22, and `wasm-bindgen-cli` 0.2.114. `just` is optional; its recipes are shortcuts for the commands below.

```sh
cargo install wasm-bindgen-cli --version 0.2.114 --locked
npm ci --ignore-scripts
# Disposable LOCAL database. These credentials are test-only.
docker run -d --name pillbox-db -p 127.0.0.1:5432:5432 \
  -e POSTGRES_PASSWORD=pillbox-local -e POSTGRES_DB=pillbox postgres:17-alpine
export DATABASE_URL='postgres://postgres:pillbox-local@localhost:5432/pillbox?sslmode=disable&search_path=public'
atlas schema apply --url "$DATABASE_URL" --to file://db/schema.sql \
  --dev-url 'docker://postgres/17/dev?search_path=public'
./scripts/build-web.sh
cargo run -p pillbox-server
```

Open **http://127.0.0.1:8080**, choose an arena, and press **PLAY NOW** for a five-minute skirmish against three server-controlled opponents. This publishes a copy of the curated arena and practice mode under your guest identity and starts an unlisted room. Bots use the same movement, ammo, reload, damage, respawn and scoring rules as players. Practice rooms can also accept friends through their invite.

For multiplayer, use a second browser/profile for a second player; tabs in one profile share the same identity. Create a server, select published versions, and share its invite. The owner starts the match. Drafts are private; published versions are immutable. The four built-in modes are editable templates in Game modes.

Controls: WASD, Space to jump, click the arena to capture the mouse, mouse to aim, left click to fire, right click to aim down sights, R to reload, 1–6 to select owned weapons, Tab for scores, Esc to release the mouse. **Keyboard aim** enables arrow keys to aim and F to fire when pointer lock is unavailable. Volume, mute, announcer and mouse sensitivity are adjustable. You start with a pistol and rifle; labels identify power weapons, supplies and objectives. Climb the reactor ramps, overlook stairs and outer sky ramps to reach three connected levels. Shields regenerate after five seconds without damage. Disconnecting and reconnecting during a match preserves health, ammo, team, scores and respawn timing. Guests retain ownership through a 30-day HttpOnly session cookie. Clearing/losing the cookie loses access to that guest identity; there is no account recovery yet.

## Architecture

```text
Browser: HTML creator/lobby UI + Bevy WASM presentation
       ↕ HTTP + WebSocket, same origin and port
One Rust Axum service
       ├── in-memory lobbies and independent 60 Hz worlds
       ├── pillbox-core: deterministic inputs, movement, combat, rules
       └── bounded async result queue → PostgreSQL
```

- `crates/pillbox-core`: serializable maps, modes, weapons, cached Box3D collision geometry, movement, combat, objectives, events and deterministic bot navigation. No renderer, IO, database, or wall-clock dependency.
- `apps/pillbox-server`: static assets, guest sessions, content ownership/versioning, discovery, owner actions, WebSockets, fixed-step simulation, durable results.
- `apps/pillbox-client`: Bevy meshes, licensed glTF models, lighting/shadows/fog, first/third-person weapons and combat effects; shared Rust movement prediction exposed to the browser. A native GPU capture harness verifies the same scene systems; a native network/UI adapter is not shipped.
- `web`: dependency-free DOM UI, canvas map editor, input collection, snapshot interpolation, radar, object labels, scoreboard, kill feed and offline positional audio/announcer.

Networking uses a deliberately small custom WebSocket protocol rather than Lightyear. Clients send input intentions, never position or damage. The server consumes at most one queued input per player per 60 Hz tick and broadcasts snapshots at 20 Hz. Inputs carry monotonic sequence numbers; snapshots acknowledge processed inputs. The browser resets to the confirmed player and replays pending inputs using the exact shared movement function. Remote transforms interpolate across a 100 ms snapshot buffer. Hitscan rewinds other players by at most 200 ms, bounded by the server's own history. Aim timestamps remain client hints within that bound. Transport is reliable/ordered, so TCP head-of-line delay remains a limitation.

Limits: 16 players/server, 64 live servers/process, four servers/owner, 512 objects/map, 32 rules/mode, bounded message sizes/queues and application request rates. Empty lobbies expire after ten minutes. Servers and active matches are intentionally ephemeral. Published content and acknowledged results survive restarts. Graceful shutdown closes sockets and attempts to persist active matches; abrupt process loss can lose an unfinished or not-yet-persisted result. Finished lobbies cannot restart until storage confirms their results.

## Build worlds and rules

Sandbox supports placing, selecting, dragging, numeric position/height/scale, yaw rotation, grid snap, duplicate/delete, draft save/load, publication, and Test/play. Load another creator's published version to fork it. A new arena starts with a floor and spawns; all other objects can be built in the browser. The top-down grid covers 50, 100, or 300 meters; the 3D preview orbits the scene.

Geometry: box, wall, floor, platform, ramp/wedge, arch, pillar/cylinder, sphere, stairs, bridge, barrier and light. Gameplay: player/team spawns, kill volumes, jump pads, teleport entrance/exit pairs, speed/gravity volumes, health/shield/weapon/ammo pickups, flags/bases, scoring zones, hills. All objects have an ID, type, position, yaw, scale and bounded typed properties, including HUD label and an optional licensed station model. Box3D supplies capsule/convex collision queries and plane solving, with swept substeps, step-up and slope following. Curves use convex facets; arches/stairs are compound hulls. Rotations are intentionally yaw-only. Scale is full dimensions; player position is feet position. Teleport targets reference exit IDs. Teams are 0 (cyan) and 1 (coral). Invalid/overlapping player spawns are rejected at save time.

CTF requires one flag base/team; optional flag primitives choose each flag's home position, otherwise its base is its home. Capture requires your flag at home and entering your team's base. Dropped flags return after 15 seconds or on teammate contact. Hills score once/second when only one team occupies the hill volumes; contested hills do not score.

Mode rules are data, never executable scripts:

```json
{
  "event": "player_killed",
  "conditions": [{ "type": "opponent" }],
  "effects": [{ "type": "add_score", "amount": 1, "team": false }]
}
```

FFA, Team Deathmatch, CTF and KOTH use this same evaluator to award points. The core supplies objective state transitions (carry, drop, return, capture, occupancy); rules consume their events. UI contains the full event/condition/effect reference and an editable JSON rules field. Every mode specifies score/time/respawn limits; configure and publish a new version to change them. Rule effects are bounded and score-change recursion is limited. Every match references explicit map and mode version UUIDs.

## Database: declarative Atlas only

`db/schema.sql` is the desired schema: users, hashed-token sessions, map/mode ownership, immutable version documents, match results and participant rows, with primary/foreign keys, uniqueness, checks and indexes. Every save inserts a new version; publication only exposes that version. Atomic database transactions serialize version numbering and save results/participants together. Result writes are idempotent by match UUID. The process uses a five-connection pool and statement/acquisition timeouts. No SQL runs in the simulation step.

**No migration system exists. There are no versioned migration files or migration directories.** Use `atlas schema apply`, `atlas schema diff`, and `atlas schema inspect`. The runtime performs no schema DDL. Atlas's own output may use the word “migration” for a declarative apply; no versioned artifact or history table is generated.

```sh
# Only a disposable local database; the script applies desired state and asserts zero drift.
TEST_DATABASE_URL="$DATABASE_URL" ./scripts/schema-check.sh
```

Shared database allocations, credentials and production schema application belong to `crvouga/workspace`. Its general guide recommends migrations; the explicit Pillbox requirement takes precedence here. Do not run schema commands against shared infrastructure until an isolated Pillbox allocation and declarative application process are approved there.

## Verification

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
npm run check
npm audit --audit-level=high
./scripts/build-web.sh                   # Separate WebGPU and WebGL2 artifacts
TEST_URL=http://127.0.0.1:8080 npm test    # Running service + disposable schema required
cargo run --release -p pillbox-core --example profile
```

The network acceptance test creates two identities, a map and custom mode, tests ownership/private drafts/immutable versions/invalid content, publishes both, creates independent rooms, connects two real WebSocket clients, starts, moves, aims/shoots, checks a kill and score, waits for durable results, restarts the match, verifies death/reconnect/respawn, disconnect and rejects invalid input. Simulation tests cover weapons, shields, health/death/respawn, objectives, map serialization, rules, collision/slopes, prediction parity, boosters, teleporters and pickup cooldowns. CI also applies the schema to disposable PostgreSQL and exercises the container.

## Container contract

```sh
docker build -t pillbox .
# The database must be reachable from inside the container, with schema already applied.
docker run --rm -p 8080:8080 -e PORT=8080 -e DATABASE_URL pillbox
curl -fsS http://127.0.0.1:8080/health
# Full acceptance on an alternate port, including assets, two clients, active-match SIGTERM and restart persistence:
DATABASE_URL='<disposable URL reachable from Docker>' ./scripts/docker-smoke.sh
```

Multi-stage Debian slim runtime, non-root UID, one port on `0.0.0.0`, no build secrets, no compiler in the runtime. Builds amd64 and arm64 with native-host WASM compilation and cross-linking when required. Build parallelism is capped to one job; allow at least 6 GB of builder memory for Bevy compilation (a 2 GB builder exhausted memory). The browser chooses WebGPU when an adapter is available and WebGL2 otherwise. Raw WASM is roughly 18 MB/backend before HTTP compression; only one backend is fetched. All assets and system fonts are self-contained.

| Environment     | Behavior                                                                                                                         |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `DATABASE_URL`  | Required opaque PostgreSQL URL. Startup fails if absent/unreachable or schema is absent.                                         |
| `PORT`          | Defaults to 8080.                                                                                                                |
| `ASSET_DIR`     | Defaults to `web`; `/app/web` in the image.                                                                                      |
| `PUBLIC_ORIGIN` | Set to the exact production `https://` origin; enforces Origin and Secure cookies. Local development checks Origin against Host. |
| `RUST_LOG`      | Rust logging filter.                                                                                                             |

Unauthenticated `/health` is process liveness and never queries the DB or third parties. HTTPS/WSS termination is external. Request bodies are bounded, SQL is parameterized, output uses text nodes, sessions use random opaque tokens (only hashes stored), owner operations are checked, and same-origin cookies plus Origin checks prevent cross-site requests. This is a bounded MVP, not a comprehensive anti-abuse service.

## Publishing and external ownership

[Current shared integration contract](https://github.com/crvouga/workspace/blob/main/llms.txt) was consulted for this implementation. `.github/workflows/publish.yml` waits for application validation and calls `crvouga/workspace/.github/workflows/ci.yml@main` with `service_id: pillbox`. The shared workflow builds/publishes `ghcr.io/crvouga/chrisvouga-pillbox:<commit-sha>` (and `latest`) and sends the prescribed `deploy-service` dispatch. Pillbox does not call Railway, own hosting resources, or apply production schema.

External prerequisites remain: approved service inventory entry for `pillbox`, an isolated app database allocation and `DATABASE_URL`, agreed declarative schema application, `PUBLIC_ORIGIN`, and infrastructure-managed `DEPLOY_DISPATCH_TOKEN`. No production deployment, external infrastructure mutation, or manual registry push is part of this implementation. See `docs/infrastructure-handoff.md`.

## MVP limits

No account recovery, moderation, matchmaking, gamepad/mobile controls, native frontend, cross-node lobby directory or arbitrary scripting. Rules are edited as JSON rather than a visual rule graph. Aim lag compensation is bounded rather than latency-proof. Player-player collision is intentionally omitted; curved geometry is faceted and ramps are yaw-only. Assists and per-player ping are not tracked (the local HUD shows acknowledgement latency). Bots currently use rifles and basic surface navigation. Licensed Kenney recordings supply the announcer until a valid ElevenLabs credential can generate the replacement pack. GPU FPS and wide-area latency require hardware/network profiling; the headless benchmark is only a CPU simulation measurement. See [the overhaul record](docs/overhaul.md) and [verification](docs/verification.md) for evidence and remaining gaps.
