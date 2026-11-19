# Importing data

The importer turns points or polygons with values into a deck. It writes
ordinary, commented cards, so the deck it makes can be read, edited and run
like any other; the deck remains the program's only input.

```bash
cargo run -- import stations.csv --value no2 > no2.sym
```

```bash
cargo run -- import wards.geojson --value density > wards.sym
```

```bash
cargo run -- wards.sym
```

In the browser, the Import data button (or dropping a file on the editor)
opens the same importer, with a choice of field, map type, width and title.
[Mapping your own data](your-own-data.md) works through an example.

## Input

**CSV** holds points. It needs a header row. Coordinates come from columns
named `lon`, `lng`, `long` or `longitude` and `lat` or `latitude`; failing
those, from `easting`, `east` or `x` and `northing`, `north` or `y`. Commas,
semicolons or tabs separate fields, and fields may be quoted.

**GeoJSON** holds a FeatureCollection (or a single Feature) of Point,
MultiPoint, Polygon, MultiPolygon, LineString or MultiLineString features;
their properties supply the values. Lines are drawn on the map (see
[Lines](#lines)).

**TopoJSON** holds the same geometries in a topology, whose arcs already
record which borders are shared. The first object is used unless
`--object` names another.

A field can be mapped if it holds numbers and no text. Without `--value`,
the first such field is used, passing over identifiers (`id`, `objectid`,
and names ending in `_id` or `_code`, for example). Items without a value
for the field are left out, with a warning. A text field called `name`,
`label`, `title`, `site`, `station` or `place` (or else one whose name
contains "name" or ends in "desc") is listed in the deck's comments, so that
each point or zone can be identified.

When a file holds both, polygons are mapped and points are left out.

## Coordinates

Longitude and latitude are projected with a transverse Mercator on WGS84,
centred on the data, in kilometres. Over a city or a county this distorts
nothing that a line printer can show. Projected coordinates are used as
given; when they span more than 1,000 units they are taken to be metres and
given in kilometres. GeoJSON and TopoJSON whose coordinates fall outside
longitude and latitude ranges are treated as projected (older GeoJSON files
in a national grid, for example).

Locations are measured down from the top edge and across from the left
edge, with a margin around the data: 5% of the larger extent for points,
2% for polygons.

## Polygons

Polygons make a conformant map by default. Each feature becomes a zone of
A-CONFORMOLINES, with one ring for each part of a MultiPolygon, and its
value goes on E-VALUES.

- **Shared borders.** A border between two zones is simplified once, so the
  zones on either side still meet and the white boundary lines stay
  continuous. For GeoJSON the importer finds the shared borders itself,
  splitting rings where neighbouring zones stop agreeing about their
  vertices (the rule TopoJSON uses); TopoJSON supplies them as arcs.
- **Simplification.** Borders are simplified (Douglas–Peucker) to within
  half a character at the chosen width, as finer detail cannot be printed.
  Parts smaller than half a character are left out, with a warning, unless
  a zone has no other part.
- **Holes and enclaves.** The first zone on the cards to claim a character
  keeps it, so smaller rings are written first, and a zone inside another
  zone's hole keeps its own level. A hole with no zone inside it (a lake,
  say) is filled by the zone around it, with a warning.

With `--contour` or `--proximal`, each zone's value is placed at its
centroid as a data point instead, and the zones themselves become the
outline, so the map stops at the edge of the data.

## Lines

Lines (LineString and MultiLineString features) carry no values. They are
drawn on a map of points or polygons, from the same file or from a second
one given with `--lines-from` (in the browser, "Lines from another file"):

```bash
cargo run -- import stations.csv --lines-from thames.geojson --lines both --line-symbol "~" > no2.sym
```

- **Use.** `--lines legend` (the default) draws them as line legends in a
  C-OTOLEGENDS package; `--lines barrier` writes them as impermeable
  barriers to interpolation in a D-BARRIERS package; `--lines both` does
  both; `--lines none` leaves them out. Barriers have no effect on a
  conformant map, so none are written for one.
- **Symbol.** `--line-symbol` sets the line legends' symbol: up to four
  characters, struck over one another (`-` unless given).
- **Placement.** Lines use the data's projection and frame; they do not
  change the map's extent. They are simplified to within half a character,
  and cut a character beyond the map's edges. A lines file must use the same
  coordinates as the data: longitude and latitude for both, or the same
  projection.

## Points

Points make a contour map by default, or a proximal map with `--proximal`.
They become B-DATA POINTS to the nearest metre, with an A-OUTLINE around
them: their convex hull, enlarged a little so that the outermost points are
not on the line. `--no-outline` maps the whole rectangle instead.

## What else the deck contains

- Three title cards: the title (the file's name unless `--title` gives one),
  the field and the number of points or zones, and the map type.
- Elective 1 for the size (8 inches wide unless `--width` says otherwise)
  and elective 2 for the extremes; electives 31, 36 and 37 for a proximal
  map.
- Comments at the head of the deck recording the source file, the
  projection and its centre, the origin of the measurements, the
  simplification, and anything left out.

## Editing the result

The imported deck uses the standard symbolism and five levels. You should
expect to adjust it: electives 3 to 6 set the levels, elective 7 the
symbols, and C-OTOLEGENDS add a title, a north arrow and a scale (see
[Writing decks](decks.md) and `examples/decks/furniture.sym`).
