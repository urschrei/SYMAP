#!/usr/bin/env bash
# Build the browser front end into target/site (served as-is by GitHub Pages).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --lib --target wasm32-unknown-unknown --profile wasm
out=target/site
rm -rf "$out"
mkdir -p "$out/examples"
cp web/index.html web/style.css web/app.js web/symap.js web/worker.js "$out/"
# Bindings and module into pkg/; the CLI must match Cargo.toml's wasm-bindgen.
wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section --out-dir "$out/pkg" \
  target/wasm32-unknown-unknown/wasm/symap.wasm
cp examples/decks/*.sym "$out/examples/"
# The user guides, rendered from docs/ (needs pandoc).
mkdir -p "$out/docs"
cp web/docs/docs.css "$out/docs/"
for doc in importing your-own-data; do
  pandoc "docs/$doc.md" --from gfm --to html5 --standalone --no-highlight \
    --template web/docs/template.html --lua-filter web/docs/links.lua \
    --output "$out/docs/$doc.html"
done
touch "$out/.nojekyll"
echo "built $out ($(du -sh "$out" | cut -f1))"
