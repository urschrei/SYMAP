# Comparison with the manual

The output has been compared with the printouts in the 1971 manual (Report
K), character by character.

## Method

The archive.org scan of the manual was rendered at 400 dpi. Each map's grid
was located from its border corners, the paper's brightness was normalised,
and each character cell was classified by template matching against the
level symbols. The result was compared with this implementation's ASA
output for the transcribed deck.

## Results

| Printout | Map | Agreement |
|----------|-----|-----------|
| Example 1-5 | Contour, from Example 1-3 | 96% of level symbols on the same level, 99% within one |
| Example 1-16 | Proximal, from Example 1-14 | Fills agree; differences only in the white lines |
| Example 1-17 | Proximal, from Example 1-14 | About 97%; differences only in the white lines |
| Example 1-22 | Conformant, from Example 1-20 | 99.4% of characters |

On Example 1-5 the numeric interpretation and the frequency distribution
match the printout exactly, and the remaining differences in the map are
balanced between higher and lower levels.

On the proximal maps every region boundary falls where the equidistance
lines between data points put it.

On the conformant map the white lines follow the zone edges themselves.
Each edge shared by two zones is traced to the nearest character on every
line it crosses (every column, for a shallow edge), whether or not the zones
are on the same level, while outer edges stay filled. Line legends are drawn
the same way.

## Open questions

- **Which side of a boundary takes the white line.** On proximal and contour
  maps the original usually blanks the character on the higher level, as
  this implementation does, but not always.
- **How outlines are filled.** On Example 1-5 the original fills slightly
  wider, from the integer part of the left edge to one past the integer
  part of the right edge. Example 1-17 shows the opposite on one island, so
  the rule is not settled.
- **The search radius for other numbers of points.** See
  [Interpolation](interpolation.md#the-search-radius).
- **Legend positions in Example 1-16.** The transcribed cards match the
  printed listing (Example 1-14), but on the printed map `SCALE:` is one row
  higher, and the scale bar two rows lower, than those cards place them. On
  this implementation's map the bar overwrites `SCALE:`. The printout may
  come from a slightly different deck than the one listed.

## Transcription notes

The manual's listings and maps were printed on different printers, and some
characters differ between them. The Example 1-14 scale-bar cards are given
as the map prints them (`I` and `+`); the listing shows `)` and a bold dot.
OCR of the scans misreads several values (search radii, `+=` as `.=`), so
every transcribed card was checked against the page images.
