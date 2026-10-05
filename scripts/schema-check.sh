#!/usr/bin/env bash
# Validate only against the disposable database supplied by the caller.
set -euo pipefail
: "${TEST_DATABASE_URL:?Set TEST_DATABASE_URL to a disposable PostgreSQL URL with search_path=public}"
atlas schema apply --url "$TEST_DATABASE_URL" --to file://db/schema.sql --dev-url 'docker://postgres/17/dev?search_path=public' --auto-approve
result=$(atlas schema diff --from "$TEST_DATABASE_URL" --to file://db/schema.sql --dev-url 'docker://postgres/17/dev?search_path=public')
printf '%s\n' "$result"
[[ "$result" == 'Schemas are synced, no changes to be made.' ]]
