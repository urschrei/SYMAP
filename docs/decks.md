# Writing decks

A deck is a sequence of 80-column cards, one per line. It follows the 1971
manual (Report K); the manual's Examples 1-3, 1-14 and 1-20 are in
`examples/decks/` and are the best starting points.

## Packages

Packages open with a header card and close with `99999` in columns 1-5. The
deck ends with `999999`. A package stays in effect for later maps until it
is replaced; an empty package removes it.

Locations are measured down from the top border and across from the left
border, in columns 11-20 and 21-30. Header columns 31-40 and 41-50 (`8.` and
`10.`) switch to row-and-column measurement, as in Example 1-3.

| Package           | Support |
|-------------------|---------|
| `A-OUTLINE`       | Any number of outlines, each closed by repeating its first vertex |
| `A-CONFORMOLINES` | Area, line and point zones; reference in columns 1-5, `A`, `L` or `P` in column 10 |
| `B-DATA POINTS`   | Complete |
| `C-OTOLEGENDS`    | Symbols, horizontal and vertical text, line and area legends, with displacement |
| `D-BARRIERS`      | Permeable and impermeable barriers, with strength varying along each barrier |
| `E-VALUES`        | Value in columns 11-20; a blank card is a blank value |
| `E1-VALUES INDEX` | Complete |
| `F-MAP`           | Three title cards, then electives |

Two extensions make decks easier to type. Lines starting with `#` between
packages are comments, and numbers separated by blanks are accepted on data
cards when the fields are not aligned.

## Electives

| Electives | Effect | Support |
|-----------|--------|---------|
| 1, 2, 13, 14, 15, 16 | Size, extreme points, scale, margins, characters per inch, size safeguard | Yes |
| 3, 4, 5, 6 | Levels, value range, equal-range, equal-count or proportional levels | Yes |
| 7 | Symbolism (five cards) | Yes |
| 8, 22 | White contour and boundary lines off, or forced | Yes |
| 9, 24 | No histogram bars, no numeric interpretation | Yes |
| 10, 11, 12 | Text, values printed at data points, repeat the previous electives | Yes |
| 17 | No table of data points or zones before the map | Yes |
| 18, 19, 20, 23 | Invalid data (`M`) | Yes |
| 21 | Keep each character's computed value (`--values`, or the site's Values download) | Yes |
| 25, 27 | Data points on conformant maps, force a contour map | Yes |
| 28 | Dot conformant maps: by level, as a share of the range, or one dot per amount | Yes |
| 31, 32, 33 | Extrapolation fraction and limits | Yes |
| 34, 35, 36 | Search radius and point count | Yes |
| 31 + 36 + 37 | Proximal map | Yes |
| 37 | Compute every character independently | Yes |
| 26 | Overprint alignment (a printer adjustment) | Accepted, no effect |
| 38, 39 | Version 6 data graph and regression plane | Not implemented (a warning) |

## Symbolism

The standard symbolism follows the manual's Tables 3-1 and 3-2. Levels run
from `.` to `O`+`X`+`A`+`V` over up to ten levels. Data points print as their
level number (`*` for the tenth). Values outside the range print as `L` and
`H`, superimposed points as `S`, invalid data as `M`, and places with no data
point within reach as `N`.

## Map furniture

SYMAP printed no titles on the map itself, nor north arrows or scale bars:
the deck drew them with C-OTOLEGENDS. Example 1-14 has the manual's simple
arrow and scale. `furniture.sym` draws the later style seen on the
Laboratory's report maps: white lettering cut through a dark area legend
(line legends with a blank symbol), and an inset cleared with a blank area
legend and ruled with line legends.

## The printed page

Each map's output starts with the map scale and a table of its data points
or zones, giving each one's printed row and column, value and level. For
contour maps it also gives the standard search radius. The map starts on a
new page, and is followed by `SYMAP`, the job time, the titles, any text,
the numeric interpretation and the frequency distribution. Maps wider than
13 inches print in panels.
