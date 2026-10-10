# External deployment handoff (prepared, not submitted)

Pillbox publishes a container; `crvouga/workspace` decides where it runs. No infrastructure has been changed by this repository.

- Repository: `crvouga/pillbox`
- Proposed service id: `pillbox` (not yet confirmed in external inventory)
- Container: `ghcr.io/crvouga/chrisvouga-pillbox:<commit-sha>`
- Dockerfile/context: `./Dockerfile`, `.`
- Port: 8080 by default; configurable through `PORT`
- Health: unauthenticated `GET /health`, no database dependency
- HTTPS/WebSocket: required; one application port; long-lived WebSocket upgrades
- Disk: none; assets baked into image, realtime worlds in memory
- Database: isolated Pillbox allocation for dev/prd, delivered through opaque `DATABASE_URL`; no existing shared allocation is assumed
- Schema: declarative desired state `db/schema.sql`; external owner must agree its Atlas application workflow. No versioned migration command exists.
- Runtime plain config: `PORT`, `PUBLIC_ORIGIN` (exact HTTPS origin), optionally `RUST_LOG`
- Secret: `DATABASE_URL` only at runtime; infrastructure-managed `DEPLOY_DISPATCH_TOKEN` for CI notification
- Shutdown: SIGTERM closes sockets, ends active worlds and attempts bounded result persistence
- Capacity: 64 logical rooms/node, up to 16 players/room; these are resource bounds, not a load-tested production capacity claim
- Publication: application validation → reusable `crvouga/workspace/.github/workflows/ci.yml@main`; standard SHA image tags and deployment dispatch

The external guide's general migration suggestion conflicts with the explicit Pillbox brief. The app contains only a desired schema. Production schema application needs to be agreed in the infrastructure repository before deployment. Do not introduce migration files to resolve that conflict.

No hosting issue was sent, no PR was pushed, no image was published and no production credentials were accessed during local implementation. Those external actions need the repository's actual CI environment and the external allocation, not invented values.

Read-only revalidation of the current service inventory confirms there is no `pillbox` service entry. The application repository has no configured repository secrets. The missing CI notification key is `DEPLOY_DISPATCH_TOKEN`; the infrastructure guide prescribes `GITHUB_TOKEN_SUPER=… bun run infra apply --set-org-dispatch-secret` from `crvouga/workspace`, performed by its owner with the real credential. Do not create or substitute credentials in this application repository. The runtime `DATABASE_URL` must come from an explicitly allocated Pillbox database, with declarative schema application agreed externally.
