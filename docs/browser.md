# The browser version

The library compiles to a WebAssembly module with bindings made by
[wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen), and `web/` is
a static front end for it, published at <https://urschrei.github.io/SYMAP/>.

## Building

```bash
./scripts/build-web.sh
```

```bash
python3 -m http.server --directory target/site
```

The build needs the `wasm-bindgen` CLI at the version `Cargo.toml` pins
(`cargo install wasm-bindgen-cli --version <that version>`). It uses the `wasm` Cargo profile (optimised for size), writes the
module and its bindings to `target/site/pkg`, and copies the front end and
the example decks into `target/site`. It also
renders [Importing data](importing.md) and [Mapping your own data](your-own-data.md)
into `target/site/docs` with [pandoc](https://pandoc.org), so those two guides
are published alongside the map.

## Interface

`src/wasm.rs` exports `run`, which takes a deck as a string and returns a
plain object (its shape is in `src/json.rs`): each map's lines (plain and per overprint pass, with
page ejects), titles, warnings, ASA output and, with elective 21, the
computed values as CSV. Errors carry the line of the card that caused them.

The front end runs the module in a Web Worker and coalesces requests, so
typing in the editor never queues more than one run. `importData` takes a
request object (the file's text, its name and the options) and returns the
deck it makes; imports use a second instance on the main thread.

## The printout

Each sheet is drawn on a canvas, one sheet per animation frame, at 10
characters and 8 lines to the inch.

- **Type.** Chain-printer type was heavy and nearly filled its cell. On the
  manual's printouts capitals are about 0.7 of the line pitch tall and 0.8
  of the character pitch wide, so the font is measured, sized and widened
  to match, and each glyph is stroked lightly for ink spread. Level fills
  then read as tones, as on the original maps.
- **Overprinting.** Every strike is printed into the same cell in
  translucent ink, with a small offset between strikes, so overprints
  darken. Each further strike on a cell is stroked a pixel wider, as more
  ink soaked into the paper, so `O`+`X`+`A`+`V` prints almost solid and
  stays distinct from `O`+`X`. This is the default, as in the original: the standard symbolism
  is built from overprints, and the manual (§5.4.2) describes the program
  printing each map line followed by up to three overprint lines. The
  Single mode shows one character per position, with modern stand-ins
  (`Θ`, `⊗`, `█`) for overprints.
- **Paper.** White, as the reports reproduced their maps (the default), or
  green-bar continuous stationery.
- **Pages.** The tables before a map are on one sheet and the map starts the
  next, as in the manual (Examples 1-4 and 1-5).
- **PNG export.** The PNG button saves each map's sheet as shown (paper,
  orientation and overprinting) at 300 dpi. At that resolution a character
  is exactly 30 pixels wide and a line 37.5 pixels tall, and the file
  records its resolution, so it prints at the original size. The map's
  titles are embedded in the file, and so, unless you turn it off in the
  Export menu, is the deck (an iTXt chunk, `SYMAP deck`): anyone with the
  file can read the cards. Dropping such a PNG on the editor, or choosing
  it with Import data, restores its deck.
- **Orientation.** Sheets can be turned a quarter-turn either way. The
  Laboratory's report maps were often printed sideways and reproduced turned
  left, with the foot of the page running up the right edge.

## Continuous integration

The `Site` workflow runs the tests, rustfmt and clippy (native and wasm),
builds the site, and runs a browser smoke test (`scripts/smoke-test.mjs`).
The smoke test renders every example, turns a sheet, imports a small CSV and a small GeoJSON,
and checks that an error links to its card. The workflow publishes `target/site` to GitHub
Pages from `main`.
