# Interpolation

`interp::Shepard` follows Shepard (1968) as implemented in SYMAP, and as
described in his 1970 report (Report L) and his 1984 chapter on SYMAP's
interpolation.

## The method

- The initial search radius satisfies π r² = 24A/N, for N data points
  occupying a rectangle of area A (see below).
- When four to ten points fall within the initial radius, it stands as the
  final radius. Otherwise the final radius becomes the distance to the point
  after the tenth, or after the fourth. When every point is used there is no
  taper.
- Weights of `1/d` taper to zero at the final radius, with a direction
  factor for points not shadowed by others.
- Each data point carries a slope, from neighbours at least 1.5 characters
  away, applied with a damping `a/(a+d)`. `a` is chosen so that the slope
  term never exceeds the elective 31 fraction (0.1) of the data range.
- Values are held within the data extremes unless electives 32 and 33 say
  otherwise.
- Barriers lengthen distances. A data point seen across a barrier an odd
  number of times is moved away by the barrier's strength (added in
  quadrature, and interpolated between vertices); an impermeable barrier
  (strength -1) removes it. `examples/decks/barrier.sym` maps a river both
  ways.

Values are computed at grid points, every third column and every second
line, and the characters between them are interpolated linearly. Elective
37 computes every character.

A grid point within half a character of data points takes their average
value. A grid point with no data within reach makes the characters derived
from it print as no data. The 1984 chapter gives 1½ characters for that
tolerance; the two make no difference on the manual's examples.

The 1970 report's worked example (two points, value 341 between them) and
the 1984 chapter's co-linear profile (plateaus beyond the extreme points, a
rise past the last one) are unit tests.

## The search radius

The documents say that the initial radius is chosen so that the circle
holds seven points on average. The program printed its radius: 3.9271 for
Example 1-3 (in Example 1-4), and 3.9958 for the Example 1-14 data points
(in Example 1-7). Seven points on average does not reproduce either value.

Both are reproduced exactly by π r² = 24A/N, where A is the area of the
rectangle the data points occupy (2.125 × 5.7 and 2.2 × 5.7) and N is their
number. Both decks have six points, so the dependence on N is inferred, not
observed; a printout with a different number of points would settle it.

Keeping the initial radius as the final radius when four to ten points fall
within it raises Example 1-5's agreement with the scan from 91% to 96%.
