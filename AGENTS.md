# Pillbox engineering

Read `build-pillbox-overnight.md` for the original acceptance criteria.
Shared infrastructure contract (fetch current): https://www.chrisvouga.dev/llms.txt

- This repository owns application code, its container, desired schema and validation/publication CI only.
- No versioned database migrations or migration directories. Atlas declarative schema operations only. `db/schema.sql` is authoritative.
- Never run application writes against a shared DB without a recorded isolated Pillbox allocation. Tests use disposable PostgreSQL.
- Pure simulation stays in `pillbox-core`; no IO/rendering/database in a tick.
- Validate authoritative rules and hostile input, and add meaningful regression tests for gameplay changes.
- Formatting, clippy with warnings denied, native tests, both WASM builds, network acceptance, Atlas zero-drift and container smoke are the intended validation gates.
- Preserve the user's PR attestation preference: inspect evidence and update truthful claims autonomously. Leave unsupported claims unchecked and record exact blockers; never fabricate proof or bypass a gate.
