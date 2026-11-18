# symap

A reimplementation of SYMAP, the Synagraphic Mapping System: Howard T.
Fisher's line-printer mapping program (Northwestern, 1963; then the Harvard
Laboratory for Computer Graphics and Spatial Analysis). It reads SYMAP card
decks and prints maps made of overprinted characters, following the 1971
program manual.

The Rust core has no dependencies. It runs from the command line or, compiled
to WebAssembly, in the browser at <https://urschrei.github.io/SYMAP/>.

```bash
cargo run -- examples/decks/example-1-14.sym
```

```bash
cargo run -- --asa examples/decks/hills.sym
```

Each map's printout starts, as the original's did, with the map scale and a
table of its data points (or zones), giving each one's printed row and
column, value and level; for contour maps it also gives the standard search
radius. `--values` prints the values computed for maps that use elective 21,
as CSV.

`--asa` writes Fortran carriage control: lines starting with `+` overprint
the line above, which is how darker tones were struck on the line printer.
Plain output stands in single characters for overprints (`Θ` for `O`+`-`,
`⊗` for `O`+`X`, `█` for `O`+`X`+`A`+`V`).

`examples/decks/example-1-14.sym` is the manual's Example 1-14 transcribed
card for card, checked against the full-resolution scan.

Checked character by character against the manual's printouts (Examples
1-16 and 1-17), the fills agree: every region boundary falls where the
equidistance lines between data points put it. On these proximal maps the
remaining differences are in which side of a boundary the white line takes:
the original usually blanks the higher-level character, as this
implementation does, but not always.

`examples/decks/example-1-20.sym` is the conformant example; its map
matches the printed Example 1-22 in 99.4% of characters. There the white
lines follow the zone edges themselves: each edge shared by two zones is
traced to the nearest character on every line it crosses (every column, for
a shallow edge), whether or not the zones are on the same level, while
outer edges stay filled. Line legends are drawn the same way.

`examples/decks/example-1-3.sym` (row-and-column measurement) produces the
contour map of Example 1-5. Its numeric interpretation and frequencies match
the printout exactly. Where both maps print a level symbol, 95% of
characters are on the same level and 99% within one, with the remaining
differences balanced between higher and lower. The original also fills outlines slightly
wider, from the integer part of the left edge to one past the integer part
of the right edge, but Example 1-17 shows the opposite on one island, so the
rule is not yet settled.

The original also printed its standard initial search radius: 3.09271 for
Example 1-3 and 3.29958 for Example 1-6 (whose data points are those of
Example 1-14). This implementation, which bases the radius on the rectangle
the data points occupy, gives 2.12; no simple measure of the points or the
outline reproduces both printed values, so the formula is still open. Using
SYMAP's printed radius (elective 34) raises Example 1-5's agreement to 97%.

## In the browser

The library compiles to a dependency-free WebAssembly module (`src/wasm.rs`:
plain C-ABI exports, results passed as JSON), and `web/` is a static front
end for it: an 80-column card editor with field guides, example decks, and a
printout that strikes each overprint layer onto green-bar paper.

```bash
./scripts/build-web.sh
```

```bash
python3 -m http.server --directory target/site
```

The `Site` workflow tests, lints and builds the site on every push and pull
request, runs a browser smoke test (`scripts/smoke-test.mjs`: every example
renders, errors link to their cards), and publishes `target/site` to GitHub
Pages from `main`.

## Decks

Packages open with a header card and close with `99999` in columns 1-5; the
deck ends with `999999`. Locations are measured down from the top border and
across from the left border, in columns 11-20 and 21-30. Header columns
31-40 and 41-50 (`8.` and `10.`) switch to row-and-column measurement. A
package stays in effect for later maps until replaced; an empty package
removes it.

| Package           | Status |
|-------------------|--------|
| `A-OUTLINE`       | any number of outlines, each closed by repeating its first vertex |
| `A-CONFORMOLINES` | area, line and point zones; reference in columns 1-5, `A`/`L`/`P` in column 10 |
| `B-DATA POINTS`   | yes |
| `C-OTOLEGENDS`    | symbol, horizontal and vertical text, line and area legends with displacement |
| `D-BARRIERS`      | permeable and impermeable barriers, strength varying along each barrier |
| `E-VALUES`        | value in columns 11-20; a blank card is a blank value |
| `E1-VALUES INDEX` | yes |
| `F-MAP`           | three title cards, then electives |

Blank-separated numbers are accepted as a fallback for hand-typed data
cards. Lines starting with `#` between packages are comments (an extension).

## Electives

| Electives | Effect | Status |
|-----------|--------|--------|
| 1, 2, 13, 14, 15, 16 | size, extreme points, scale, margins, characters per inch, size safeguard | yes |
| 3, 4, 5, 6 | levels, value range, equal-range, equal-count or proportional levels | yes |
| 7 | symbolism (five cards) | yes |
| 8, 22 | white contour and boundary lines off, or forced | yes |
| 9, 24 | no histogram bars, no numeric interpretation | yes |
| 10, 11, 12 | text, print values at data points, repeat previous electives | yes |
| 18, 19, 20, 23 | invalid data (`M`) | yes |
| 25, 27 | data points on conformant maps, force contour | yes |
| 31, 32, 33 | extrapolation fraction and limits | yes |
| 34, 35, 36 | search radius and point count | yes |
| 31 + 36 + 37 | proximal map | yes |
| 37 | compute every character independently | yes |
| 17 | no table of data points or zones before the map | yes |
| 21 | keep each character's computed value (`--values`, or the site's Values download) | yes |
| 28 | dot conformant maps: by level, as a share of the range, or one dot per amount | yes |
| 26 | overprint alignment (a printer adjustment) | accepted, no effect |
| 38, 39 | Version 6 data graph and regression plane | not implemented (warning) |

Standard symbolism follows Tables 3-1 and 3-2: from `.` to `O`+`X`+`A`+`V`
over up to ten levels, data points as their level number (`*` for the
tenth), `L` and `H` outside the value range, `S` for superimposed points,
`M` for invalid data, `N` where no data point is within reach. The page
below the map gives the titles, text, numeric interpretation and frequency
distribution in the manual's layout. Maps wider than 13 inches print in
panels.

## Interpolation

`interp::Shepard` follows Shepard (1968) as implemented in SYMAP and
described in his 1970 report and 1984 chapter on SYMAP's interpolation:

- the initial search radius holds about seven points on average, based on
  the rectangle the data points occupy; the radius then adjusts to keep four
  to ten points, and the final radius is the distance to the next data
  point (no taper when every point is used);
- weights of `1/d` taper to zero at the final radius, with a direction
  factor for points not shadowed by others;
- each data point carries a slope, from neighbours at least 1.5 characters
  away, applied with a damping `a/(a+d)`; `a` is chosen so the slope term
  never exceeds the elective 31 fraction (0.1) of the data range;
- values are held within the data extremes unless electives 32 and 33 say
  otherwise.
- barriers lengthen distances: a data point seen across a barrier an odd
  number of times is moved away by the barrier's strength (added in
  quadrature, interpolated between vertices), and an impermeable barrier
  (strength -1) removes it. `examples/decks/barrier.sym` maps a river both
  ways.

Values are computed at grid points, every third column and every second
line, and the characters between them are interpolated linearly; elective 37
computes every character. A grid point within half a character of data
points takes their (average) value, and a grid point with no data within
reach makes the characters derived from it print as no data. (The 1984
chapter gives 1½ characters for that tolerance; the two make no difference
on the manual's examples.) The 1970 report's worked example (two points,
value 341 between them) and the 1984 chapter's co-linear profile (plateaus
beyond the extreme points, a rise past the last one) are unit tests.

## References

- Goodrich, J. C. (ed.) and Howrigan, G. (assoc. ed.) (1971). *Computer
  Mapping as an Aid in Air Pollution Studies*, Vol. 2, Report K: Computer
  Program Manuals. Laboratory for Computer Graphics and Spatial Analysis,
  Graduate School of Design, Harvard University. NTIS PB 204 713
  (APTD-0738). Scan: <https://archive.org/details/micro_IA41159411_0506>
- Shepard, D. (1968). A two-dimensional interpolation function for
  irregularly-spaced data. *Proceedings of the 1968 23rd ACM National
  Conference*, 517–524. <https://doi.org/10.1145/800186.810616>
- Shepard, D. S. (1970). *SYMAP Interpolation Characteristics*. Computer
  Mapping as an Aid in Air Pollution Studies, Vol. 2, Report L. Laboratory
  for Computer Graphics and Spatial Analysis, Harvard University. NTIS PB 204
  180 (APTD-0787). Scan: <https://archive.org/details/micro_IA41159411_0474>
- Shepard, D. (1984). Computer Mapping: The SYMAP Interpolation Algorithm.
  In G. L. Gaile & C. J. Willmott (eds), *Spatial Statistics and Models*,
  Springer, 133–145. <https://doi.org/10.1007/978-94-017-3048-8_7>

## Licence

[Blue Oak Model License 1.0.0](LICENSE.md).
