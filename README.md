# symap

A reimplementation of SYMAP, the Synagraphic Mapping System: Howard T.
Fisher's line-printer mapping program (Northwestern, 1963; then the Harvard
Laboratory for Computer Graphics). It reads SYMAP-style card decks and
prints maps made of overprinted characters.

This is the Rust core. It has no dependencies and is meant to be compiled to
WebAssembly for a browser front end.

```bash
cargo run -- examples/decks/hills.sym
```

```bash
cargo run -- --asa examples/decks/hills.sym
```

`--asa` writes Fortran carriage control: lines starting with `+` overprint
the line above, which is how darker tones were struck on the line printer.

## Map types

| Kind       | Chosen when                                         | Each character cell gets         |
|------------|-----------------------------------------------------|----------------------------------|
| Contour    | default with data points                            | Shepard interpolation            |
| Proximal   | `*TYPE PROXIMAL`                                    | value of the nearest data point  |
| Conformant | default when `A-CONFORMOLINES` is present           | value of the enclosing zone      |

Cells are scaled in inches, so non-square printer cells (default 10
characters and 6 lines per inch) keep the map's proportions.

## Deck format

Packages open with a header card and close with `99999`. The deck ends with
`999999`. Coordinates are `down across`, measured from the top-left corner.
Numbers may be blank-separated or in consecutive 10-column fields (`F10.x`).
Lines beginning with `#` are comments (an extension).

| Package           | Cards                                                                 | Status |
|-------------------|-----------------------------------------------------------------------|--------|
| `A-OUTLINE`       | one vertex per card; cells outside are left blank                     | documented |
| `B-DATA POINTS`   | one point per card                                                    | documented |
| `E-VALUES`        | one value per card, in data-point (or zone) order                     | documented |
| `F-MAP`           | three title cards, then electives                                     | documented |
| `A-CONFORMOLINES` | `zone down across`; a ring closes when it repeats its first vertex    | provisional |
| others (`C-`, `D-`) | skipped with a warning                                              | not yet |

Electives put their number in columns 4-5 and their values in 10-column
fields from column 11:

| Elective | Fields (columns 11-20, 21-30, ...)       | Status |
|----------|------------------------------------------|--------|
| 1        | map height, width (inches)               | implemented |
| 13       | scale                                    | parsed, not applied |
| 14       | margins: top, left, bottom, right        | implemented |
| 15       | lines per inch, characters per inch      | implemented |

Behaviour whose original elective numbers are not yet confirmed uses
`*KEYWORD` cards instead, so they cannot clash with real electives:
`*TYPE CONTOUR|PROXIMAL|CONFORMANT`, `*LEVELS n` (1-10, default 5),
`*BREAKS b0 b1 ... bn`, `*MARKPOINTS` (print data points as their level
digit), `*NOSLOPE`, `*NODIRECTION`.

The package names, terminators, the three title cards, the elective column
layout, and electives 1, 14 and 15 come from a contemporary user guide
(NOAA, *BATHYGRAPH*, which prepares SYMAP input). The conformoline card
layout, the default tone ramp and the legend layout are provisional until
they can be checked against the SYMAP user's reference manual.

## Interpolation

`interp::Shepard` follows Shepard (1968):

- a search radius holding about 7 points on average, adjusted to keep 4 to 10;
- weights of `1/d` that taper to zero at the radius;
- a direction factor that favours points not hidden behind others;
- a slope correction along locally estimated gradients.

The result passes exactly through the data. With `*NOSLOPE` it also stays
within the data range.

## References

- Shepard, D. (1968). A two-dimensional interpolation function for
  irregularly-spaced data. *Proceedings of the 1968 23rd ACM National
  Conference*, 517–524. <https://doi.org/10.1145/800186.810616>
- Shepard, D. (1984). Computer Mapping: The SYMAP Interpolation Algorithm.
  In G. L. Gaile & C. J. Willmott (eds), *Spatial Statistics and Models*,
  Springer, 133–145. <https://doi.org/10.1007/978-94-017-3048-8_7>
- NOAA. *BATHYGRAPH: a program package for interdepartmental use* (SYMAP
  input conventions). <https://repository.library.noaa.gov/view/noaa/37800>
