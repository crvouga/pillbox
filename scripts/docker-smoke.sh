#!/usr/bin/env bash
set -euo pipefail
: "${DATABASE_URL:?Supply a disposable database URL reachable from the container}"
image=${1:-pillbox}
name="pillbox-smoke-$$"
cleanup() {
  status=$?
  if [[ $status != 0 ]]; then docker logs "$name" >&2 || true; fi
  docker rm -f "$name" >/dev/null 2>&1 || true
  exit "$status"
}
trap cleanup EXIT
network_args=()
if [[ -n ${DOCKER_NETWORK:-} ]]; then network_args=(--network "$DOCKER_NETWORK"); fi
# An alternate port proves PORT is honored. Bind only to localhost.
docker run -d --name "$name" "${network_args[@]}" -p 127.0.0.1:18081:8081 -e PORT=8081 -e PUBLIC_ORIGIN= -e DATABASE_URL "$image" >/dev/null
for _ in {1..60}; do
  if curl -fsS http://127.0.0.1:18081/health >/dev/null; then break; fi
  sleep 1
done
curl -fsS http://127.0.0.1:18081/health
curl -fsS http://127.0.0.1:18081/ | rg 'PILLBOX' >/dev/null
curl -fsS http://127.0.0.1:18081/pkg/webgpu/pillbox_client_bg.wasm -o /dev/null
curl -fsS http://127.0.0.1:18081/pkg/webgl/pillbox_client_bg.wasm -o /dev/null
curl -fsS http://127.0.0.1:18081/assets/models/weapons/blaster-e.glb -o /dev/null
curl -fsS http://127.0.0.1:18081/assets/models/station/container.glb -o /dev/null
curl -fsS http://127.0.0.1:18081/assets/models/station/Textures/colormap.png -o /dev/null
curl -fsS http://127.0.0.1:18081/assets/audio/laserSmall_001.m4a -o /dev/null
curl -fsS http://127.0.0.1:18081/assets/audio/voice/fight.m4a -o /dev/null
TEST_URL=http://127.0.0.1:18081 npm test
TEST_URL=http://127.0.0.1:18081 SMOKE_CONTAINER="$name" node --test tests/shutdown.test.js
docker stop --time 20 "$name" >/dev/null
[[ $(docker inspect -f '{{.State.ExitCode}}' "$name") == 0 ]]
printf '\nContainer assets, HTTP/WebSocket acceptance, PORT and SIGTERM passed.\n'
