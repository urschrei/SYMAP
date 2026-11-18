#!/usr/bin/env bash
# Build the browser front end into target/site (served as-is by GitHub Pages).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --lib --target wasm32-unknown-unknown --profile wasm
out=target/site
rm -rf "$out"
mkdir -p "$out/examples"
cp web/index.html web/style.css web/app.js web/symap.js web/worker.js "$out/"
cp target/wasm32-unknown-unknown/wasm/symap.wasm "$out/"
cp examples/decks/*.sym "$out/examples/"
touch "$out/.nojekyll"
echo "built $out ($(du -sh "$out" | cut -f1))"
