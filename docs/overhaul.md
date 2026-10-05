# Playability overhaul

2026-10-05. Deployment is outside this work's scope.

## Playing

Run the service and open `http://127.0.0.1:8080`. **PLAY NOW** creates an unlisted
five-minute, fifteen-elimination practice match with three authoritative bots.
Choose Terraces, Crosscut or Courtyard: grid-built arenas with different middle
routes, 8 m decks and 16 m overlooks. See [map design](map-design.md). Friends can join
through the room invite. The host form also supports zero to four opponents.

Use WASD, Space, mouse aim, left click to fire, right click to aim down sights,
R to reload, 1–6 for owned weapons, and Tab for scores. **Keyboard aim** provides
arrow-key aiming and F to fire when pointer lock is unavailable. Sound starts
after the join gesture; volume, mute, announcer and sensitivity controls remain
available in the match.

## Implemented changes

- Pure core movement uses cached `box3d-rust` convex hulls, capsule queries and
  plane solving, swept substeps, step assistance and downhill following. The
  server and WASM prediction use the same controller. Curved primitives are
  faceted; stairs and arches are compound colliders. Hitscan retains analytical
  map ray tests. Players do not collide with each other.
- Arenas use seven fixed building pieces, face-aligned anchors, 4 m horizontal
  cells, 1 m height increments and quarter turns. Structural cover and supports
  replace decorative props. Two 8 m-wide ramp routes reach levels at 0, 8 and
  16 m; middle decks leave 7 m of clearance. The editor and server share the
  catalog and enforce snapping, fixed sizing and non-overlap.
- Every map receives shadowed daylight and ambient fill. Matte surfaces share
  materials for repeated kit meshes. There are no emissive architectural strips,
  painted floor lines, station dressing or pickup pedestals.
- All six carried weapons have licensed models in first and third person;
  first-person models show recoil, movement bob, reload motion and ADS. Shot
  tracers end at the authoritative wall impact; explosions have a brief visual.
- HUD adds object context, distance labels for visible players/supplies/objectives,
  radar, hit/kill markers and damage feedback. Labels use text nodes and are
  hidden behind geometry or when pickups are on cooldown.
- Positional action sound covers weapon fire, impacts, footsteps, jump, landing,
  gravity lifts, reload, switching, dry fire, teleport, pickups, shield break/
  recharge, respawn, death/explosions and objective transitions. Two quiet engine
  loops provide ambience. Announcements duck effects and use offline recordings.
- Deterministic bots plan surface routes and send ordinary movement/fire/reload
  inputs. They have no damage or ammunition bypass. They currently favor rifles.
  Room capacity counts bots, and results retain their scores as JSON participants.
  Surface probes and graph size are bounded for dense creator geometry; this
  navigation aid targets local arenas rather than arbitrarily large worlds.

## Announcer blocker

The authorized `ELEVEN_LABS_API_KEY` fields in Vault `secret/personal/dev` and
`secret/personal/prd` were read selectively from `https://vault.chrisvouga.dev`.
Both were rejected by ElevenLabs `GET /v2/voices` with HTTP 401,
`invalid_api_key`. A direct dev-key TTS generation request also returned HTTP 401.
No successful ElevenLabs generation is claimed, and no key
was written to the repository, client, logs or image.

The replacement generator is ready:

```sh
python3 scripts/generate-announcer.py --config dev --voice-id YOUR_VOICE_ID
```

It uses an injected `ELEVEN_LABS_API_KEY` or reads that single Vault field, creates
sixteen finite MP3 callouts with `eleven_multilingual_v2`, and publishes a local
manifest only after generation succeeds. Existing generated clips are reused.
Until a working key is available, licensed Kenney voice recordings replace the
previous browser speech synthesis. Assets and license provenance are recorded
in [assets](../web/assets/README.md).

## Verification

Portable commands are in the repository README. The overhaul adds regression
coverage for the grid kit, map spawn support/headroom, rotated thin-wall sweeps, ascending and
descending stairs, actual controller traversal of every arena's three levels,
navigation to all power weapons, and sustained bot combat. HTTP/WebSocket
acceptance additionally checks bot capacity, hostile bot configuration, combat,
respawns, shot endpoints and durable bot scores. Container smoke fetches models,
texture atlases, audio and both WASM backends before running the network suite.

Native GPU captures use the same Bevy app, scene assets, cameras and render
systems as the browser. They require every scene model and dependency to load
before taking the image and fail if loading does not finish. To reproduce:

```sh
cargo run --locked -p pillbox-client --example capture -- /tmp/pillbox-fps.png fps 0 rifle
cargo run --locked -p pillbox-client --example capture -- /tmp/pillbox-arena.png overview 0
```

Browser WebGPU initialization, licensed model rendering, HUD, sound decoding,
practice join/start, bot kills and durable standings were observed through the
documented `sc browser` interface. Its background tab intermittently reports
`visibilityState=hidden`, suspending animation. This is not proof of sustained
browser FPS, a manual mouse-aim playtest or a two-person WAN playtest. Native
GPU captures and protocol/controller tests provide independent evidence while
that browser limitation remains. ElevenLabs generation is the external blocker.
