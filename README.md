# symap

A reimplementation of SYMAP, the Synagraphic Mapping System: Howard T.
Fisher's line-printer mapping program (Northwestern, 1963; then the Harvard
Laboratory for Computer Graphics and Spatial Analysis). It reads SYMAP card
decks and prints maps made of overprinted characters, following the 1971
program manual.

This is the Rust core. It has no dependencies and is meant to be compiled to
WebAssembly for a browser front end.

```bash
cargo run -- examples/decks/example-1-14.sym
```

```bash
cargo run -- --asa examples/decks/hills.sym
```

`--asa` writes Fortran carriage control: lines starting with `+` overprint
the line above, which is how darker tones were struck on the line printer.
Plain output stands in single characters for overprints (`Θ` for `O`+`-`,
`⊗` for `O`+`X`, `█` for `O`+`X`+`A`+`V`).

`examples/decks/example-1-14.sym` is the manual's Example 1-14 transcribed
card for card, checked against the full-resolution scan.

Checked character by character against the manual's printouts (Examples
1-16 and 1-17), the fills agree: every region boundary falls where the
equidistance lines between data points put it. The remaining differences
are in which side of a boundary the white line takes. The original usually
blanks the higher-level character, as this implementation does, but on some
boundaries it blanks the lower one, and no rule tested yet explains both.

`examples/decks/example-1-3.sym` (row-and-column measurement) produces the
contour map of Example 1-5. Its numeric interpretation and frequencies match
the printout exactly. Where both maps print a level symbol, 95% of
characters are on the same level and 99% within one, with the remaining
differences balanced between higher and lower. The original also fills outlines slightly
wider, from the integer part of the left edge to one past the integer part
of the right edge, but Example 1-17 shows the opposite on one island, so the
rule is not yet settled.

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
| `D-BARRIERS`      | parsed; interpolation ignores them (with a warning) |
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
| 17, 26 | tabular output, overprint alignment | accepted, no effect |
| 21, 28, 38, 39 | tape output, dot maps, data graph, regression plane | not implemented (warning) |

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
