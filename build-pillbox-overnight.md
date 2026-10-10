# Build Pillbox Overnight

You are the primary engineer responsible for taking this repository from its current state to a working production-ready MVP of **Pillbox**, a browser-first multiplayer arena shooter.

Work autonomously for the entire run.

Do not stop to ask questions unless you are absolutely blocked by missing credentials or an external system that cannot be mocked or worked around.

When requirements are ambiguous, make the simplest reasonable decision that preserves the architecture and product intent described below.

Keep working until the definition of done is satisfied as completely as possible.

Do not merely scaffold features.

Implement them.

Test them.

Run them.

Fix failures.

Continue iterating until the system works end-to-end.

---

# Product

Pillbox is a lightweight browser-based multiplayer arena shooter inspired by classic Halo-style sandbox gameplay.

The visual identity is intentionally simple.

Players are pills/capsules.

Worlds are composed primarily of simple geometric primitives.

Think:

- pills vs boxes
- strange shape universes
- interdimensional arenas
- playful science-fiction
- colorful surreal environments
- simple geometry
- simple skyboxes
- Adventure Time / Rick and Morty energy
- intentionally stylized rather than realistic

The simplicity is intentional and should be treated as the actual art direction, not placeholder art.

---

# Core product philosophy

The engine provides primitives.

Users create the content.

Users create:

- multiplayer servers
- maps
- game modes
- matches

The application should contain as little hardcoded authored gameplay content as possible.

Maps and modes are data interpreted by the engine.

Do not introduce arbitrary user scripting in the initial implementation.

Prefer declarative composition.

---

# Definition of done

A user must be able to:

1. Open the browser application.
2. Create a multiplayer server.
3. Configure that server.
4. Select a map.
5. Select a game mode.
6. Other users can discover or join that server.
7. Multiple players can join.
8. Players can leave.
9. A multiplayer match can begin.
10. Players can move and shoot one another.
11. Gameplay feels like a basic classic Halo-style arena shooter.
12. Health, shields, death, respawning and scoring work.
13. Scoreboard works.
14. Kill feed works.
15. Announcer events work.
16. Match completion works.
17. Durable match results are persisted.
18. Users can enter the sandbox editor.
19. Users can create a map entirely in the browser.
20. Users can place gameplay primitives.
21. Users can save that map.
22. Users can publish/use that map.
23. Users can create or configure a game mode.
24. Users can save that mode.
25. Users can create another multiplayer server using their newly created map and mode.
26. Other users can join that server.
27. The user-created map and mode can be played successfully.

The product is not complete until that loop works end-to-end.

---

# Required technology

Prefer the following stack unless the repository already contains a clearly superior compatible implementation.

## Game/client

- Rust
- Bevy
- browser target via WASM
- WebGPU where supported
- browser-first

The native Bevy target should remain possible later without redesigning the core game.

## Multiplayer

Use an authoritative server.

Preferred stack:

- Bevy
- Lightyear
- WebSocket transport for the browser-facing initial deployment

Implement:

- authoritative server state
- client-side prediction
- reconciliation
- interpolation of remote players
- lag compensation where appropriate
- reproducible simulation where practical

Do not make PostgreSQL part of the realtime simulation loop.

---

# Infrastructure ownership boundary

This repository does **not own infrastructure**.

All infrastructure, hosting, deployment destinations, networking, DNS, secrets, databases and cloud-provider configuration are owned externally by:

`crvouga/workspace`

Integration guide:

`https://github.com/crvouga/workspace/blob/main/llms.txt`

Read that guide before implementing deployment integration.

Treat it as the authoritative contract.

If this repository's assumptions conflict with the current `crvouga/workspace` infrastructure definition, the infrastructure repository wins.

Do not duplicate infrastructure definitions here.

---

# Infrastructure responsibilities of this repository

This repository owns only:

- application source code
- game source code
- browser client
- server runtime
- Dockerfile
- automated tests
- declarative database schema
- CI needed to validate/build the project
- image publication integration required by `crvouga/workspace`

This repository does **not** own:

- Railway resources
- Railway service definitions
- Railway deployment configuration
- DNS
- TLS
- Postgres provisioning
- cloud networking
- production secrets
- Vault/OpenBao infrastructure
- deployment-region selection
- deployment topology
- infrastructure OpenTofu
- database infrastructure

Do not add OpenTofu/Terraform for those things here.

Do not run `railway up`.

Do not configure Railway manually.

Do not create provider-specific deployment files unless the shared infrastructure contract explicitly requires them.

---

# Deployment model

The application repository produces a container image.

That is the deployment artifact.

Conceptually:

```text
Pillbox repository
       |
       | CI
       v
Docker image
       |
       v
GHCR
       |
       | deployment notification
       v
crvouga/workspace
       |
       v
personal infrastructure chooses deployment
```

The application must not care where the container ultimately runs.

It should run anywhere satisfying its runtime contract.

---

# Shared infrastructure contract

Follow the current `crvouga/workspace/llms.txt` instructions.

The current managed-hosting model expects applications to be Dockerized and published as prebuilt GHCR images. The infrastructure repository then owns deployment, including Railway resources and health checking.

Do not reinvent this workflow.

Use the reusable workflow or integration contract defined there.

The application repository's job ends after:

1. validating the application
2. building the Docker image
3. publishing the expected immutable image tag
4. notifying the infrastructure workflow as defined by the shared contract

The infrastructure repo decides what happens next.

---

# Container contract

Create a production-quality `Dockerfile`.

Requirements:

- multi-stage build
- build tools only in build stages
- minimal runtime image
- builds with normal `docker build`
- no secrets required at image build time
- Linux amd64 must build successfully
- runtime configuration comes from environment variables
- listen on `0.0.0.0`
- honor `$PORT`
- sensible default port for local development
- provide an unauthenticated lightweight health endpoint
- fast startup
- graceful shutdown

The current personal infrastructure contract expects a single exposed application port, environment-based configuration, a health endpoint, and a prebuilt GHCR image.

Verify locally:

```bash
docker build -t pillbox .
docker run --rm -p 8080:8080 -e PORT=8080 pillbox
curl -fsS http://localhost:8080/health
```

Adjust the health path only if the shared infrastructure contract requires another path.

---

# Server

Run one Rust service initially.

That service should support:

- serving the browser client/assets
- HTTP API
- multiplayer WebSocket connections
- multiplayer server/lobby management
- server discovery
- multiple simultaneous logical matches
- authoritative game simulation

Multiple matches should run inside the same process.

Do not create one process/container per match.

---

# Architecture

Favor a very simple architecture.

Runtime topology:

```text
Browser
   |
   | HTTPS / WSS
   v
Pillbox container
   |
   +-- static WASM/web assets
   +-- HTTP API
   +-- multiplayer server browser
   +-- lobby lifecycle
   +-- authoritative matches
   +-- multiple matches in memory
   |
   v
PostgreSQL
```

The deployment environment itself is external and not owned here.

PostgreSQL stores durable state only.

Live match state belongs in memory.

A server restart may terminate currently running matches.

That is acceptable for the MVP.

The architecture should make it straightforward for the personal infrastructure layer to run more than one node later without rewriting core simulation logic.

---

# Repository architecture

Prefer clean bounded crates/modules.

A reasonable conceptual structure is:

```text
apps/
  pillbox-server/
  pillbox-web/

crates/
  pillbox-core/
  pillbox-network/
  pillbox-map/
  pillbox-modes/
  pillbox-client/
  pillbox-server-runtime/

db/
  schema/

.github/
  workflows/

Dockerfile
```

Do not create:

```text
infra/
migrations/
terraform/
opentofu/
railway/
```

unless something in those locations is explicitly required by the external integration contract.

Do not force the exact application package layout if the repository already has a good established convention.

The critical boundary is:

```text
shared game logic
      |
      +-- browser client
      |
      +-- authoritative server
```

Share simulation types and rules wherever practical.

---

# Database philosophy

Use PostgreSQL for durable state.

Use Atlas for **declarative schema management only**.

## Absolutely no migrations

Do not create versioned SQL migrations.

Do not create an Atlas migration directory.

Do not use:

```text
atlas migrate
atlas migrate diff
atlas migrate apply
migration files
timestamped SQL migrations
```

Do not generate migrations as an intermediate artifact.

Do not introduce migration-based tooling.

The desired database schema itself is the source of truth.

---

# Atlas schema

Maintain a declarative desired schema such as:

```text
db/
  schema.hcl
```

or another Atlas-supported declarative schema format appropriate for the project.

The intended model is:

```text
desired schema
      |
      v
Atlas
      |
      v
actual database schema
```

not:

```text
schema
  -> migration
  -> migration history
  -> database
```

Use Atlas capabilities appropriate to declarative schema workflows, such as:

```bash
atlas schema inspect
atlas schema diff
atlas schema apply
```

where applicable.

The exact production schema application mechanism may be owned by `crvouga/workspace`.

Do not assume this application repository owns production schema application unless the external infrastructure contract explicitly says so.

This repository must define the desired schema and validate it.

---

# Database ownership boundary

The application owns the desired logical schema.

The personal infrastructure owns:

- database provisioning
- connection credentials
- production secret delivery
- deployment environment
- production schema application workflow if defined there

Keep these concerns separate.

Runtime database connectivity must be configurable by environment variables.

Do not hardcode database hostnames or credentials.

---

# Durable schema

Design a sensible schema for at least:

- users
- maps
- map versions
- game modes
- game mode versions
- multiplayer servers if durable definitions are needed
- match results
- match players
- player statistics if needed

Use:

- primary keys
- foreign keys
- unique constraints
- check constraints
- indexes

Published map and game-mode versions should be immutable from the product's perspective.

Do not store transient realtime simulation state in PostgreSQL.

---

# Player representation

Players are pills/capsules.

Do not spend time on humanoid models.

A player needs at minimum:

- position
- orientation
- velocity
- aim direction
- grounded state
- shield
- health
- active weapon
- ammunition
- team
- score
- alive/dead state

Use primitive collision shapes.

Prefer a purpose-built predictable arena-shooter character controller over force-driven rigid-body movement.

Implement:

- walking/running
- air movement
- jumping
- gravity
- ramps/slopes
- collision
- respawn

Keep movement simple and responsive.

---

# Combat

Implement a small expressive weapon set.

At minimum:

- pistol
- automatic rifle
- precision rifle
- shotgun
- sniper
- rocket launcher

Support appropriate properties:

- damage
- fire rate
- magazine size
- reload
- spread
- projectile velocity
- splash radius
- ammo
- respawn delay

Use hitscan where appropriate.

Use projectiles where appropriate.

Server validates combat.

Client predicts immediate local feedback.

---

# Shields and health

Implement:

- shields
- health
- shield depletion
- shield regeneration
- death
- respawn
- kill attribution

Keep mechanics simple and readable.

---

# Sandbox map editor

The sandbox is a first-class product feature.

Users must be able to build an entire playable map in the browser.

Implement:

- place
- select
- move
- rotate
- scale where appropriate
- duplicate
- delete
- save
- load
- test/play

Add snapping where useful.

Prioritize usability and correctness over polish.

---

# Required map primitives

## Geometry

- box
- wall
- floor
- platform
- ramp
- wedge
- arch
- pillar
- cylinder
- sphere
- stairs
- bridge
- barrier

## Player/gameplay

- player spawn
- team spawn
- kill volume

## Movement

- booster/jump pad
- teleporter entrance
- teleporter exit
- speed boost
- gravity modifier if practical

## Pickups

- health pickup
- shield pickup
- weapon spawn
- ammo pickup

## Objectives

- flag
- flag base
- scoring zone
- hill/control zone

Every primitive must have a complete serializable representation.

Maps must be reproducible entirely from their saved data.

---

# Map model

Maps are durable versioned data.

Conceptually:

```text
Map
  metadata
  version
  skybox
  objects[]
```

Objects contain:

```text
type
position
rotation
scale
properties
```

Do not store arbitrary executable user code.

Use primitive IDs and properties.

Published map versions should be immutable.

Matches should reference explicit map versions.

---

# Game modes

Ship with:

## Free For All

- everyone is an opponent
- kills award score
- configurable score limit
- configurable time limit

## Team Deathmatch

- teams
- team score
- configurable score limit
- configurable time limit

## Capture the Flag

- team flags
- pickup
- carrying
- dropping
- returning
- capture
- configurable capture limit

## King of the Hill

- hill zone
- contested state
- ownership
- score accumulation
- configurable score/time target

---

# User-created game modes

Do not implement arbitrary scripting.

Implement a declarative rules system:

```text
event
  -> conditions
  -> effects
```

For example:

```text
ON player_killed
IF killer != victim
DO killer.score += 1
```

Or:

```text
ON player_enters_zone
IF player.carrying_flag
AND zone.team == player.team
DO capture_flag
DO team.score += 1
```

Possible events:

- match_started
- player_spawned
- player_killed
- player_entered_zone
- player_left_zone
- player_picked_up_item
- player_dropped_item
- timer_elapsed
- score_changed

Possible conditions:

- player team
- player state
- item state
- zone ownership
- score threshold
- timer threshold

Possible effects:

- add score
- end round
- end match
- respawn player
- give weapon
- remove item
- reset object
- announce event
- change objective state

Implement built-in modes using this same system wherever reasonably practical.

Avoid a second hardcoded mode engine.

---

# Multiplayer server model

Users create logical multiplayer servers.

Each supports:

- name
- owner
- visibility
- max players
- selected map version
- selected game mode version
- game mode parameters
- current players
- lifecycle state

Lifecycle:

```text
waiting
starting
playing
finished
```

Users must be able to:

- create server
- browse servers
- join server
- leave server
- start match
- finish match
- reuse server for another match

---

# Realtime simulation

Live state stays in memory:

- transforms
- velocity
- health
- shields
- ammo
- projectiles
- pickups
- objectives
- timers
- scores

Never use PostgreSQL as a per-tick synchronization mechanism.

Persist durable results after or around match completion.

---

# Scoreboard

Implement an in-match scoreboard.

At minimum:

- player
- team where relevant
- score
- kills
- deaths
- assists if supported
- objective score if supported
- ping if available

Also provide a post-match result.

---

# Kill feed

Display recent combat events.

At minimum:

```text
Player A eliminated Player B
```

Include weapon information where practical.

---

# Announcer

Implement semantic announcer events.

Gameplay emits events such as:

- match_starting
- match_started
- victory
- defeat
- double_kill
- triple_kill
- killing_spree
- flag_taken
- flag_dropped
- flag_returned
- flag_captured
- hill_controlled
- hill_contested
- one_minute_remaining
- thirty_seconds_remaining
- match_over

Presentation consumes these semantic events.

If voice assets do not exist, browser speech synthesis or temporary simple audio is acceptable.

Preserve the event architecture.

---

# Visual direction

Keep rendering intentionally cheap.

Prefer:

- primitive meshes
- simple materials
- flat or restrained lighting
- minimal shadows
- bright colors
- surreal skyboxes
- strong team readability
- lightweight effects

Avoid:

- skeletal characters
- realistic humanoids
- dense meshes
- expensive materials
- unnecessary shaders
- heavy postprocessing

---

# Performance targets

Aim for:

- responsive controls
- stable simulation
- 60 FPS on ordinary modern desktop hardware
- 120 FPS on capable hardware
- reasonable WASM size
- low draw-call counts
- simple collision
- minimal hot-loop allocation
- no DB work in the simulation tick

Profile before deep optimization.

---

# Networking behavior

Local input should feel immediate.

Do not wait for server acknowledgement before:

- moving the local player
- showing immediate weapon feedback

The server remains authoritative.

Implement/configure:

- prediction
- reconciliation
- interpolation
- appropriate snapshot/update rates
- lag compensation where appropriate

Never trust client-reported damage as authoritative.

---

# Security

Treat clients as hostile.

Validate:

- movement
- fire rates
- ammo
- weapon ownership
- damage
- pickups
- scoring
- map ownership
- mode ownership
- server-owner actions

Do not build a giant anti-cheat system.

Preserve server authority.

---

# Testing philosophy

The repository should be confidently testable without routine manual testing.

Add tests while implementing.

## Unit tests

At minimum cover:

- scoring
- weapons
- shields
- health
- respawn
- map serialization
- mode rules
- teleporters
- boosters
- objectives

## Simulation tests

Run core gameplay without rendering.

Test:

- joins
- leaves
- movement
- death
- respawn
- multiple players
- match transitions
- game modes
- objectives

## Networking tests

Test:

- connect
- disconnect
- multiple clients
- state replication
- authoritative corrections
- server shutdown behavior where practical

## Database/schema tests

Test:

- desired Atlas schema parses successfully
- schema can be applied to a fresh disposable PostgreSQL database
- resulting schema matches the desired state
- map persistence works
- mode persistence works
- match result persistence works

Again: do this with declarative Atlas schema application.

Do not generate or test migrations.

## End-to-end

Automate:

```text
create map
create mode
create server
player A joins
player B joins
start match
players move
player A kills player B
score changes
match completes
result persists
```

Use browser automation if appropriate, but keep fast headless simulation tests for game logic.

---

# Deterministic/headless simulation

Core game logic should run without rendering wherever practical.

Tests should be capable of something conceptually like:

```text
create world
load map
add players
feed input
advance ticks
assert state
```

Do not couple gameplay rules unnecessarily to renderer APIs.

---

# CI

CI should prove the application is healthy.

At minimum run:

- formatting
- linting
- native Rust compilation
- WASM compilation
- unit tests
- simulation tests
- networking tests
- Atlas schema validation
- disposable Postgres schema-apply test
- Docker build
- container smoke test
- health endpoint smoke test

Then use the publishing mechanism required by `crvouga/workspace`.

---

# Image publishing

The application repository should publish the prebuilt Docker image exactly according to the personal infrastructure contract.

Do not invent a separate deployment mechanism.

The current infrastructure guide defines a reusable workflow where the application repo builds and pushes the expected GHCR image and dispatches deployment responsibility to `crvouga/workspace`.

Follow that contract.

Do not:

- deploy Railway directly
- configure Railway directly
- use Railway source builds
- use Nixpacks
- push images manually as the normal deployment flow
- duplicate the infrastructure workflow

---

# Runtime configuration

All environment-specific values must come from environment variables.

Examples may include:

```text
PORT
DATABASE_URL
RUST_LOG
```

Use the exact names expected by the personal infrastructure contract when specified there.

Never commit production secrets.

Never bake secrets into the image.

Fail fast with a useful error if required runtime configuration is missing.

---

# Health endpoint

Provide a simple health endpoint such as:

```text
GET /health
```

Requirements:

- no authentication
- returns 2xx quickly
- does not depend on third-party internet services
- usable by external infrastructure health checks

Do not make basic liveness depend on optional external integrations.

---

# Local development

Local development should be straightforward.

Aim for a small command surface.

For example:

```bash
just dev
just test
just build
just build-web
just schema-apply
just docker-build
just docker-smoke
```

or equivalent.

For local PostgreSQL:

```text
start disposable/local Postgres
       |
       v
atlas schema apply
       |
       v
run application
```

No migration step exists.

---

# Documentation

Update the README with:

- what Pillbox is
- architecture
- local development
- testing
- declarative Atlas schema workflow
- Docker image contract
- integration boundary with `crvouga/workspace`
- sandbox content model
- game-mode rules system
- multiplayer architecture

Clearly state:

> This repository does not own production infrastructure or hosting. It publishes a deployable container image; `crvouga/workspace` owns deployment.

Also document known limitations honestly.

---

# Implementation strategy

Work vertically.

Do not spend the entire run designing abstractions.

Get this working early:

```text
browser
-> join server
-> spawn pill
-> move
-> shoot
-> kill
-> respawn
-> score
```

Then expand outward.

Recommended order:

1. Inspect the repository.
2. Read `crvouga/workspace/llms.txt`.
3. Understand the existing infrastructure integration contract.
4. Identify reusable existing application architecture.
5. Establish shared simulation.
6. Implement player movement.
7. Implement one weapon.
8. Implement authoritative multiplayer.
9. Get two clients playing.
10. Implement server create/join/leave.
11. Add shield/health/death/respawn.
12. Add score and match lifecycle.
13. Define declarative Atlas schema.
14. Verify schema against disposable Postgres.
15. Add durable application persistence.
16. Add sandbox map model.
17. Add sandbox editor.
18. Add primitives.
19. Add declarative game-mode rules.
20. Implement FFA.
21. Implement Team Deathmatch.
22. Implement Capture the Flag.
23. Implement King of the Hill.
24. Add scoreboard.
25. Add kill feed.
26. Add announcer events.
27. Add remaining weapons.
28. Harden multiplayer.
29. Create production Dockerfile.
30. Add container smoke test.
31. Add CI.
32. Integrate image publication with the personal infrastructure contract.
33. Run the complete acceptance flow.
34. Audit.
35. Fix defects.
36. Repeat until stable.

---

# Autonomous working rules

Do not stop after making a plan.

Do not stop after scaffolding.

Do not stop after compiling once.

Do not ask the user to manually test something that can be automated.

Do not introduce TODOs for straightforward work if there is time to implement them.

Do not rewrite working systems without reason.

Continuously test.

When something fails:

1. investigate
2. find the root cause
3. fix it
4. rerun focused tests
5. rerun broader tests

If an approach is failing, change approaches.

Prefer working vertical functionality over theoretical elegance.

---

# Critical prohibitions

## No database migrations

Do not create them.

Do not generate them.

Do not recommend adding them later during this run.

Do not create an empty migrations directory.

Do not use Atlas migrate commands.

The desired declarative schema is authoritative.

## No infrastructure ownership

Do not define hosting infrastructure here.

Do not add Railway resources.

Do not add Terraform/OpenTofu infrastructure for the application.

Do not manipulate shared infrastructure.

Do not attempt to own DNS, TLS, Vault, Postgres infrastructure or Railway.

## No direct production deployment

This repository publishes an image.

`crvouga/workspace` owns deployment.

Respect that boundary.

---

# Decision-making rules

When choosing between:

```text
generic
```

and:

```text
simple and sufficient
```

choose simple and sufficient.

When choosing between:

```text
polish
```

and:

```text
working gameplay
```

choose working gameplay.

When choosing between:

```text
hardcoded authored content
```

and:

```text
reusable primitive
```

prefer the primitive.

When choosing between:

```text
application-owned infra
```

and:

```text
external personal infra contract
```

use the external infrastructure contract.

When choosing between:

```text
migration-based database evolution
```

and:

```text
declarative desired schema
```

always use declarative desired schema.

---

# Non-goals

Do not spend major time on:

- realistic models
- skeletal animation
- story
- NPCs
- large open worlds
- vehicles
- destructible environments
- advanced terrain
- arbitrary user meshes
- arbitrary user scripts
- photorealism
- professional audio production
- elaborate cosmetics
- monetization
- large social systems

---

# Completion audit

Before ending, perform a serious audit.

Verify:

- server create/join/leave
- multiple simultaneous players
- disconnect handling
- movement
- shooting
- kill/death
- respawn
- score
- match start/end
- map save/load
- map publication
- user-created modes
- invalid map data
- invalid mode data
- duplicate joins
- spawning inside invalid geometry
- CTF resets
- KOTH contested state
- weapon cooldown exploits
- schema correctness
- fresh declarative schema application
- no migrations anywhere
- WASM build
- native/server build
- Docker image build
- container boot
- `$PORT` behavior
- health endpoint
- no secrets in image/repo
- image-publishing workflow compatibility
- no application-owned infrastructure configuration
- no accidental renderer coupling in core simulation

Search the repository specifically for migration artifacts and remove them.

Search for infrastructure ownership that belongs in `crvouga/workspace` and remove it.

Repeat the audit after fixes.

---

# Final output

At the end of the run, provide:

## Implemented

What genuinely works.

## Architecture

Important boundaries and choices.

## Gameplay

What players can actually do.

## Sandbox

What users can create.

## Database

Describe the declarative Atlas schema and how it was validated.

Explicitly confirm that no migration system exists.

## Container

How to build and run the production image locally.

## Infrastructure integration

Describe how the image publication hooks into `crvouga/workspace`.

Do not claim this repository deploys infrastructure.

## Tests

List what was run and its result.

## Remaining gaps

Anything genuinely incomplete.

## Recommended next steps

Only the highest-value remaining work.

Never present planned work as finished work.

---

# Primary objective

By the end of this run, this repository should contain the smallest robust implementation of:

> **A browser-based multiplayer arena shooter where users create the servers, maps and game modes from a strong set of geometric and gameplay primitives.**

Two people should be able to open Pillbox, join the same user-created server, play a user-created map under a configurable mode, shoot each other as pills inside a surreal world of shapes, complete the match, and then create another experience entirely through the sandbox.

The application repository should produce a tested immutable Docker image.

The personal infrastructure repository should remain the sole owner of where and how that image is hosted.

Keep working toward that outcome until the run ends.