#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --release --locked -p web --target wasm32-unknown-unknown
rm -rf web/site
mkdir -p web/site/glyphs
cp web/index.html web/site/
cp -r web/brand web/site/
cp target/wasm32-unknown-unknown/release/web.wasm web/site/
cp tests/font/font_*.png web/site/glyphs/
