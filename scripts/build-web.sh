#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
for backend in webgpu webgl; do
  cargo build --locked -p pillbox-client --target wasm32-unknown-unknown --release --no-default-features --features "$backend"
  wasm-bindgen --target web --out-dir "web/pkg/$backend" target/wasm32-unknown-unknown/release/pillbox_client.wasm
done
