# Importing points

The importer turns points with values into a deck. It writes ordinary,
commented cards, so the deck it makes can be read, edited and run like any
other; the deck remains the program's only input.

```bash
cargo run -- import stations.csv --value no2 > no2.sym
```

```bash
cargo run -- no2.sym
```

In the browser, the Import data button (or dropping a file on the editor)
opens the same importer, with a choice of field, map type, width and title.

## Input

**CSV** needs a header row. Coordinates come from columns named `lon`,
`lng`, `long` or `longitude` and `lat` or `latitude`; failing those, from
`easting`, `east` or `x` and `northing`, `north` or `y`. Commas, semicolons
or tabs separate fields, and fields may be quoted.

**GeoJSON** needs a FeatureCollection (or a single Feature) of Point or
MultiPoint features; their properties supply the values. Other geometries
are left out, with a warning. Polygons and lines are not imported yet.

A field can be mapped if it holds numbers and no text. Without `--value`,
the first such field is used, passing over identifiers such as `id` and
`objectid`. Points without a value for the field are left out, with a
warning. A text field named `name`, `label`, `title`, `site`, `station` or
`place` is listed in the deck's comments, so that each data point can be
identified.

## What the deck contains

- **Coordinates.** Longitude and latitude are projected with a transverse
  Mercator on WGS84, centred on the points, in kilometres. Over a city or a
  county this distorts nothing that a line printer can show. Projected
  coordinates are used as given; when they span more than 1,000 units they
  are taken to be metres and given in kilometres. GeoJSON whose coordinates
  fall outside longitude and latitude ranges is treated as projected.
- **Measurement.** Locations are measured down from the top edge and across
  from the left edge, with a margin of 5% of the larger extent around the
  points.
- **Outline.** An A-OUTLINE around the points: their convex hull, enlarged a
  little so that the outermost points are not on the line. `--no-outline`
  maps the whole rectangle instead.
- **Data points and values.** B-DATA POINTS to the nearest metre, and
  E-VALUES as given.
- **Map.** Three title cards (the title, the field and number of points, and
  the map type), elective 1 for the size (8 inches wide unless `--width`
  says otherwise) and elective 2 for the extremes. `--proximal` adds
  electives 31, 36 and 37 for a proximal map.

The comments at the head of the deck record the source file, the projection
and its centre, the origin of the measurements, and anything left out.

## Editing the result

The imported deck uses the standard symbolism and five levels. You should
expect to adjust it: electives 3 to 6 set the levels, elective 7 the
symbols, and C-OTOLEGENDS add a title, a north arrow and a scale (see
[Writing decks](decks.md) and `examples/decks/furniture.sym`).
