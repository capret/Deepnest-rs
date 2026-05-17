#!/usr/bin/env bash
# Build the NFP geometry engine (crates/nfp) to WebAssembly and regenerate the
# wasm-bindgen glue under frontend/nfp/.
#
# One-time prerequisites:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.100
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "==> compiling crates/nfp to wasm32-unknown-unknown (release)"
cargo build --manifest-path crates/nfp/Cargo.toml \
  --target wasm32-unknown-unknown --release

echo "==> generating JS glue with wasm-bindgen (no-modules)"
wasm-bindgen \
  --target no-modules \
  --no-typescript \
  --out-dir frontend/nfp \
  --out-name nfp \
  crates/nfp/target/wasm32-unknown-unknown/release/nfp.wasm

echo "==> done: frontend/nfp/nfp.js + frontend/nfp/nfp_bg.wasm"
