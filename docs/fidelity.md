# Comparison with the manual

The output has been compared with the printouts in the 1971 manual (Report
K), character by character.

## Method

`scripts/compare_scan.py` renders a page of the archive.org scan of the
manual at 400 dpi, turns it upright, divides out the paper's brightness and
fits the map's four border lines (or takes the corners from the command
line, for Example 1-17, whose dashed fill hides the border). Each character
cell is cut from the grid between the corners and classified by template
matching against blank and the map's level symbols, the templates being
refined from the scan itself. The result is compared with this
implementation's output for the transcribed deck; letters, digits and
legends are left out.

```bash
scripts/compare_scan.py manual.pdf 32 examples/decks/example-1-3.sym \
    --bbox 1395,593,4795,2726 --diff 1-5.txt
```

| Printout | Page | Deck | Options |
|----------|------|------|---------|
| Example 1-5 | 32 | `example-1-3.sym` | `--bbox 1395,593,4795,2726` |
| Example 1-16 | 52 | `example-1-14.sym` | `--bbox 1259,820,4661,2955` |
| Example 1-17 | 54 | `example-1-14.sym` | `--map 1 --corners 1040,695,4449,650,1075,2830,4484,2780 --levels=-█` |
| Example 1-22 | 62 | `example-1-20.sym` | `--bbox 1102,601,4492,2732` |

## Results

| Printout | Map | Characters agreeing | Level symbols on the same level |
|----------|-----|---------------------|---------------------------------|
| Example 1-5 | Contour, from Example 1-3 | 94.3% | 98.6% (all within one) |
| Example 1-16 | Proximal, from Example 1-14 | 98.9% | 99.8% |
| Example 1-17 | Proximal, from Example 1-14 | 99.5% | 100% |
| Example 1-22 | Conformant, from Example 1-20 | 99.4% | 99.8% |

On Example 1-5 the numeric interpretation and the frequency distribution
match the printout exactly. The remaining differences in the map are
balanced between higher and lower levels (7 and 9 characters), and most
are where the level-5 area is a column or two wider in the original, which
is interpolation rather than drawing.

On the proximal maps every region boundary falls where the equidistance
lines between data points put it, and the fills agree everywhere.

Outlines are filled where a character's centre is inside them. Every row
of all four printouts agrees with that rule; widening the fill by half a
character or a whole one on either side lowers agreement on every map.

### White lines

On contour and proximal maps, lines are made in printing order: a
character becomes a line where its level differs from the character above
it, or from the one to its left unless that one is already a line. So each
boundary takes one character, on whichever side is reached second. Lines
that would leave no symbol between them are dropped, as the manual
describes for standard output (elective 22 keeps them).

This replaces blanking the character on the higher level, which put the
line on the wrong side wherever the higher level lies above and to the
right, as on the boundary between the 10 and 20 zones in Examples 1-16 and
1-17. The change raised agreement from 91.5% to 94.3% on Example 1-5, from
98.4% to 98.9% on Example 1-16 and from 99.1% to 99.5% on Example 1-17. The
remaining differences on the proximal maps are mostly at the ends of steps
in shallow boundaries, where the original puts the line one column further
right.

On the conformant map the white lines follow the zone edges themselves.
Each edge shared by two zones is traced to the nearest character on every
line it crosses (every column, for a shallow edge), whether or not the zones
are on the same level, while outer edges stay filled. Line legends are drawn
the same way.

## Open questions

- **Steps in shallow proximal boundaries.** See above.
- **The search radius for other numbers of points.** See
  [Interpolation](interpolation.md#the-search-radius).
- **Legend positions in Example 1-16.** The transcribed cards match the
  printed listing (Example 1-14), and the legend columns match the printout,
  but the rows do not. The cards put `SCALE:` on row 10 and the scale bar on
  row 10, its labels on row 9, so the bar overwrites `SCALE:`. The printout
  has `SCALE:` on row 9, the labels on row 11 and the bar on row 12, which
  cards with the bar at 1.5 inches down instead of 1.3, and `SCALE:` shifted
  4 lines instead of 5, would give. The printout probably comes from a
  slightly different deck than the one listed.

## Transcription notes

The manual's listings and maps were printed on different printers, and some
characters differ between them. The Example 1-14 scale-bar cards are given
as the map prints them (`I` and `+`); the listing shows `)` and a bold dot.
OCR of the scans misreads several values (search radii, `+=` as `.=`), so
every transcribed card was checked against the page images.
