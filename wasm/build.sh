#!/bin/sh
# Builds the npm package in wasm/npm (dataviz.wasm next to index.js).
set -e
cd "$(dirname "$0")/.."
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
# SIMD128: every current browser, Node 16.4+, Deno and Bun.
RUSTFLAGS="-C target-feature=+simd128" \
  cargo build -p dataviz-wasm --profile wasm --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/wasm/dataviz_wasm.wasm wasm/npm/dataviz.wasm
if command -v wasm-opt >/dev/null; then
  wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-simd wasm/npm/dataviz.wasm -o wasm/npm/dataviz.wasm
fi
ls -l wasm/npm/dataviz.wasm
