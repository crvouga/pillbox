# Verification record

Local unattended implementation, 2026-10-05. Evidence is kept outside tracked files in `/Users/chrisvouga/.cache/pillbox-overnight-641a/` for this session; portable checks live in this repository.

## Observed passing checks

- Pure Rust simulation: 24 integration tests covering movement, rotated aim, slope jumping/collision, shared prediction parity, all six weapons, hitscan cover, rocket travel, shield delay/regeneration, damage/death/respawn, scoring/time limits, CTF capture/drop/return/reset/disconnect, contested KOTH, pickups, zones, kill volumes, teleporters, jump pads, map validation/serialization, mode validation/rules, duplicate joins, lobby reuse, reconnect state/respawn timing, projectile friendly-fire after owner departure, and departed-owner kill credit/score-limit completion.
- Workspace native tests and native client compilation passed after the final gameplay additions.
- Formatting and clippy (`--workspace --all-targets -- -D warnings`) passed, including the Bevy native client.
- Both release WASM builds and wasm-bindgen output generated successfully (WebGPU and WebGL2).
- Real HTTP/WebSocket acceptance passed repeatedly (including server configuration ownership and occupied-server protection): create user content, publication/immutability/ownership, two clients, state replication, bounded movement, shooting/kill/score, match completion, database result, rematch, disconnect, two simultaneously playing room isolation, dead-player reconnect/authoritative respawn and hostile input/origin checks.
- Durable match `9fbd5c74-60ad-44e5-a0df-3bd4b117d00b` was retrieved after stopping the service and starting a new process.
- Atlas `schema apply` created the desired schema on disposable PostgreSQL 17. `schema diff` reported exactly `Schemas are synced, no changes to be made.` No migration artifacts or history were generated.
- Node syntax checks passed. The test WebSocket dependency was updated to 8.22.0 after audit; `npm audit` reports zero vulnerabilities.
- Browser shell, guest session, sandbox navigation, new map, primitive placement, duplicate/delete, map rename/publication and mode publication were exercised through the documented `sc browser` interface. DOM reported both version 1 published and immutable. A server using these exact user-created versions was created, joined, started and finished through the browser, and its result was saved. A visible WebGPU screenshot confirmed correctly colored arena geometry after the tone-mapping fix. A further browser check verified the final scoreboard lists saved participants after both original players have left. Pointer-lock play remains limited by foreground focus in automation.
- Release CPU probe: 16 players, 3,600 ticks on the starter map completed in 0.44 s (0.122 ms/tick, 136.6× realtime) on this development machine. This is not a GPU frame-rate or multi-room capacity guarantee.

- Final Linux amd64 Docker build passed. Container acceptance passed for static assets, both WASM bundles, HTTP/WebSocket creator/combat flow, alternate `PORT=8081`, same-origin/hostile-origin handling, active-match SIGTERM, socket closure, exit 0 and result recovery after restart. The shutdown fix passed three focused consecutive runs; after reconnect/attribution fixes, the final image passed the complete suite and shutdown/restart test again. Native idle SIGTERM exited 0 in 0.009 s.
- Runtime inspection confirms UID/GID 65532, both WASM assets, no Cargo/Rust/GCC, and no database URL or secret in image configuration.

## Verified image

- Local tag: `pillbox:verified-578a509913af` (also `pillbox:latest`)
- Immutable image ID: `sha256:578a509913afa90105ae6a7d392bb0aaaaaa0de915618f29fa4d1db04b342d4d`
- Platform: `linux/amd64`
- Local archive: `/Users/chrisvouga/.cache/pillbox-overnight-641a/pillbox-linux-amd64-578a509913af.tar`
- Archive SHA-256: `8ac01071f4f0f849cafcf386a058249971d3ea59faee0d9632b69c0ffc0055c7`
- Container-created shutdown result recovered after restart: `ea9231f6-1a5f-45d4-84d6-47336adeb1ad`. The earlier focused repeat runs also recovered their results and exited cleanly.
- This is a locally verified artifact. No registry publication is claimed.

## Failures found and repaired

- Reconnecting reset health, ammo, team and score, and bypassed death delay: resume the departed participant in the active match, reset only transport input acknowledgements, and count participation once.
- Rockets lost their team/kill-credit context when owners disconnected: retain the firing team in projectiles and evaluate kill/scoring rules for retained departed participants. A departing owner can still reach the score limit.
- The final browser scoreboard omitted departed players and could announce the wrong FFA outcome: load the durable participant list for final standings and victory/defeat.

- Ramp stepping raised feet too far and broke grounded jumping: place feet on the actual surface; regression test added.
- Yaw sign for movement disagreed with camera aim: shared controller corrected; 90-degree test added.
- WebSocket writer originally ignored an inner send failure: handle timeout and socket errors.
- Active-match SIGTERM test exposed lost results: the writer could observe shutdown before the next simulation tick queued the final result. A second test found retained HTTP state kept sender clones alive. The ticker now explicitly signals producer completion; the writer then closes and drains its receiver and reports timeout/persistence failures. This also avoids an idle-worker shutdown hang.
- Bevy's default tone mapping required an omitted lookup-texture feature: use `Tonemapping::None`.
- Invalid-spawn regression originally retained a duplicate ID, allowing a false positive: isolate the geometry case and assert its specific rejection message.
- Blank `PUBLIC_ORIGIN` from the example environment was treated as an exact empty origin: normalize blank values to local Host validation; same-origin acceptance is now checked alongside hostile-origin rejection.
- Missing cross-architecture libc headers: add only in the Docker build stage when cross-compiling.
- Unbounded parallel compilation exhausted the local 2 GB Docker VM and killed local forwarding/database processes: cap Docker compilation to one job. Both interrupted containers were restored (the unrelated local container reports healthy). The standard Docker socket was restored through the existing Lima SSH master without restarting the VM. A subsequent single-job build still exceeded 2 GB, so the final build uses a dedicated 6 GB local VM; no production infrastructure is involved.

## Evidence gaps

- The in-app browser often reports `document.visibilityState === "hidden"`; its animation frames and screenshots can be suspended/stale. Mouse pointer lock also requires actual foreground focus. Do not treat DOM interaction or headless simulation as proof of sustained visible-frame performance or a manual two-person playtest.
- No production infrastructure, credentials, GHCR publication, remote CI run or production deployment was exercised. Read-only GitHub inspection confirms the remote repository is empty and its repository-secret listing is empty. The current external fleet has no Pillbox allocation; read-only inspection of the authoritative OpenTofu service inventory also confirms no `pillbox` entry. The caller inputs/secrets were also compared directly with the current reusable workflow and match its schema.

## Definition-of-done audit

| Brief criteria | Implemented path and observed evidence |
| --- | --- |
| 1: open browser | Real WebGPU renderer initialized; visible screenshot shows the arena and browser shell. |
| 2–5: create/configure server, select map/mode | Browser host/configuration form; HTTP acceptance verifies explicit version IDs, owner-only changes and occupied-server protection. |
| 6–8: discover, join, multiple players, leave | Public listing and unlisted invite behavior; two real WebSocket identities, duplicate rejection, replicated joins and disconnect removal. |
| 9–10: begin, move, shoot | Waiting → starting → playing transitions; acknowledged bounded movement; two-client aimed combat. |
| 11: arena-shooter feel | Shared predicted movement, jump/air control, six weapon classes, shields and respawns are implemented. Subjective feel and WAN latency are not established by headless tests. |
| 12: health/shield/death/respawn/score | Core regression tests plus a real replicated kill and scored finish. |
| 13–15: scoreboard, kill feed, announcer | Browser presentation consumes authoritative player stats and semantic events; network assertions prove kill and match-over events. Speech/pointer-lock require browser capabilities and foreground interaction. |
| 16–17: completion, durable result | Score/time/owner completion; persisted result with explicit content versions and participant rows; result retrieved across native process restart. |
| 18–22: browser sandbox, create/place/save/publish | Browser-created floor/spawn/box map, numeric editing, duplicate/delete, save and immutable v1 publication exercised through `sc browser`. Serialization/invalid-spawn tests cover reproducibility and validation. |
| 23–24: create/configure/save mode | Browser mode form and declarative rules editor; custom mode published in browser and acceptance test. Unsupported executable effects rejected. |
| 25–27: new server using user content, other players join/play | Browser-created map/mode used in a browser-created match that saved its result. Independently, the automated HTTP/WebSocket creator flow publishes custom content, connects two players, completes combat and checks stored results. A manual two-person pointer-lock playtest is not claimed. |

The completion audit also checks CTF reset/drop/return, contested hills, weapon cooldown/ownership, invalid geometry and mode data, isolated simultaneous worlds, native and WASM compilation, fresh schema application, and renderer-free core dependencies. Source/artifact searches found no migration directories/files, infrastructure definitions, embedded private keys or token patterns, or unimplemented/TODO stubs. Local test passwords are explicitly disposable examples.
