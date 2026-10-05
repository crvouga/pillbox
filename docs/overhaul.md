# Playability overhaul

2026-10-05. Deployment is outside this work's scope.

## Playing

Run the service and open `http://127.0.0.1:8080`. **PLAY NOW** creates an unlisted
five-minute, fifteen-elimination practice match with three authoritative bots.
Choose Foundry Nine, Helix Station or Blacksite Relay; they share a connected
three-level arena plan with different orientations and colors. Friends can join
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
- Arenas have elevated bases, ground maintenance rooms, cargo and console cover,
  reactor ramps, overlook stairs, upper gantries, crossing bridges, outer sky
  ramps, gravity lifts, supplies and power weapons. Walkable levels are 0, 3 and
  6 meters. Outer ramps and landing gaps avoid overhead bridges and railings.
- Bevy renders global shadowed directional light, placed colored lights,
  emissive architectural trims, fog, floor grid panels and licensed station
  models. The sandbox can place lights, labels and station props.
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
coverage for map spawn support/lighting, rotated thin-wall sweeps, ascending and
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
