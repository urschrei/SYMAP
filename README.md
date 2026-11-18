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
card for card. Its second map closely matches Example 1-17 (Town
Precincts): the precinct layout agrees by eye, and the level ranges,
percentages and frequencies agree exactly with the original printout.

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
| 9, 24 | no frequency chart, no numeric interpretation | yes |
| 10, 11, 12 | text, print values at data points, repeat previous electives | yes |
| 18, 19, 20, 23 | invalid data (`M`) | yes |
| 25, 27 | data points on conformant maps, force contour | yes |
| 31, 32, 33 | extrapolation fraction and limits | yes |
| 34, 35, 36 | search radius and point count | yes |
| 31 + 36 + 37 | proximal map | yes |
| 17, 26, 37 | tabular output, overprint alignment, independence | accepted (every character is computed independently) |
| 21, 28, 38, 39 | tape output, dot maps, data graph, regression plane | not implemented (warning) |

Standard symbolism follows Tables 3-1 and 3-2: from `.` to `O`+`X`+`A`+`V`
over up to ten levels, data points as their level number (`*` for the
tenth), `L` and `H` outside the value range, `S` for superimposed points,
`M` for invalid data, `N` where no data point is within reach. The page
below the map gives the titles, text, numeric interpretation and frequency
distribution in the manual's layout. Maps wider than 13 inches print in
panels.

## Interpolation

`interp::Shepard` follows Shepard (1968): a search radius that holds about
seven points on average, adjusted to keep four to ten; weights of `1/d`
tapering to zero at the radius; a direction factor; and a slope correction
limited to a fraction (0.1) of the data range. The surface passes exactly
through the data and is held within the data extremes unless electives 32
and 33 say otherwise.

The manual says standard SYMAP computes about one character in six and
derives the rest, unless elective 37 asks for every character to be computed
independently. This implementation always computes every character.

## References

- Goodrich, J. C. (ed.) and Howrigan, G. (assoc. ed.) (1971). *Computer
  Mapping as an Aid in Air Pollution Studies*, Vol. 2, Report K: Computer
  Program Manuals. Laboratory for Computer Graphics and Spatial Analysis,
  Graduate School of Design, Harvard University. NTIS PB 204 713
  (APTD-0738). Scan: <https://archive.org/details/micro_IA41159411_0506>
- Shepard, D. (1968). A two-dimensional interpolation function for
  irregularly-spaced data. *Proceedings of the 1968 23rd ACM National
  Conference*, 517–524. <https://doi.org/10.1145/800186.810616>
- Shepard, D. (1984). Computer Mapping: The SYMAP Interpolation Algorithm.
  In G. L. Gaile & C. J. Willmott (eds), *Spatial Statistics and Models*,
  Springer, 133–145. <https://doi.org/10.1007/978-94-017-3048-8_7>

## Licence

[Blue Oak Model License 1.0.0](LICENSE.md).
