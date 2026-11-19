# The browser version

The library compiles to a dependency-free WebAssembly module, and `web/` is
a static front end for it, published at <https://urschrei.github.io/SYMAP/>.

## Building

```bash
./scripts/build-web.sh
```

```bash
python3 -m http.server --directory target/site
```

The build uses the `wasm` Cargo profile (optimised for size) and copies the
module, the front end and the example decks into `target/site`. It also
renders [Importing data](importing.md) and [Mapping your own data](your-own-data.md)
into `target/site/docs` with [pandoc](https://pandoc.org), so those two guides
are published alongside the map.

## Interface

`src/wasm.rs` exports a plain C ABI: `symap_alloc`, `symap_run`,
`symap_import`, `symap_result_ptr` and `symap_free`. A deck goes in as UTF-8 and the result
comes back as JSON: each map's lines (plain and per overprint pass, with
page ejects), titles, warnings, ASA output and, with elective 21, the
computed values as CSV. Errors carry the line of the card that caused them.

The front end runs the module in a Web Worker and coalesces requests, so
typing in the editor never queues more than one run. `symap_import` takes a
JSON request (the file's text, its name and the options) and returns the
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
  darken. This is the default, as in the original: the standard symbolism
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
