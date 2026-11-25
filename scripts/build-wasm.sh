#!/bin/sh
set -e
cd "$(dirname "$0")/.."
cargo build --release --target wasm32-unknown-unknown -p entityres-wasm
cp target/wasm32-unknown-unknown/release/entityres_wasm.wasm bindings/js/entityres.wasm
cp target/wasm32-unknown-unknown/release/entityres_wasm.wasm bindings/go/entityres.wasm
