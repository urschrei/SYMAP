# Mapping your own data

This page takes one dataset from a CSV file to a printed map, then gives
shorter recipes for other kinds of data. The example is the annual mean
concentration of nitrogen dioxide (NO2) at 71 monitoring sites in and
around London in 2025, in `examples/data/london-no2-2025.csv`. The finished
deck is `examples/data/london-no2-2025.sym`.

## Choosing the map

SYMAP draws three kinds of map, and the data decide which one fits.

| Your data | Map | What it shows |
|-----------|-----|---------------|
| Values measured at points (monitoring sites, spot heights) | Contour | A continuous surface interpolated between the points |
| Points whose areas of influence matter (stations, schools) | Proximal | Each character takes the value of the nearest point |
| Values for areas (boroughs, census districts) | Conformant | Each area filled at its own level |

NO2 is measured at points and varies continuously, so the example is a
contour map. A conformant map of the same data would need areas, and a
proximal map would show which site is nearest, not how the concentration
varies.

## Importing the data

The CSV has a header row, then one site per line:

```text
site,code,type,longitude,latitude,no2
Bexley - Belvedere,BX2,Suburban Background,0.15891,51.49061,15.5
Bexley - Belvedere West,BQ7,Urban Background,0.13728,51.49465,14.3
Borehamwood Meadow Park,BDMP,Urban Background,-0.27055,51.66123,12.4
```

The importer finds the coordinates from the column names and maps the only
numeric field, `no2`:

```bash
cargo run -- import examples/data/london-no2-2025.csv > no2.sym
```

In the browser, Import data (or dropping the file on the editor) does the
same, with a choice of field, map type, width and title. [Importing
data](importing.md) describes what the importer accepts.

## Reading the deck

The deck opens with comments. They record the source, the projection and
the origin of the measurements, and list the sites in the order of the
data cards:

```text
# Made by symap import from london-no2-2025.csv: 71 points, mapping no2.
# Longitude and latitude are projected with a transverse Mercator on WGS84,
# centred on -0.0827, 51.4806, in kilometres.
# Locations are measured down from 23.843 and across from -32.086, with a
# margin of 2.919 around the points.
#
# Data points, in order:
#    1 Bexley - Belvedere
#    2 Bexley - Belvedere West
```

Four packages follow, each opened by a header card and closed by `99999`.

- **A-OUTLINE** encloses the area to map: here the convex hull of the
  sites, enlarged a little. Characters outside it stay blank.
- **B-DATA POINTS** gives each site's location, measured *down* from the
  top of the map in columns 11–20 and *across* from the left in columns
  21–30, in kilometres.
- **E-VALUES** gives each site's value in columns 11–20, in the same order.
- **F-MAP** asks for a map: three title cards, then electives. Elective 1
  sets the map's size in inches (5.93 down, 8.0 across), and elective 2
  the extent of the source coordinates that the map covers.

```text
B-DATA POINTS
                22.7     48.87
              22.256    47.366
               3.728     19.09
```

The editor's ruler and field guides show the columns. `Tab` moves to the
next field.

Running the deck prints the map. The tables before it give each site's
printed row and column, value and level, which is a useful check that the
locations are where you expect.

## Setting the levels

The imported deck uses SYMAP's standard five levels, of equal size between
the lowest and highest values (12.4 and 47.7). Levels at round values are
easier to read. Electives 3 to 5 set four levels of 10 between 10 and 50,
so that the top level holds the sites above the UK's annual objective of
40:

```text
F-MAP
LONDON - NITROGEN DIOXIDE, ANNUAL MEAN 2025
MICROGRAMS PER CUBIC METRE AT 71 MONITORING SITES (DEFRA UK-AIR)
CONTOUR MAP - SHEPARD INTERPOLATION
    1           5.93       8.0
    2            0.0       0.0    47.614    64.213
    3             4.
    4            10.
    5            50.
99999
```

The elective number goes in columns 1–5 and its value in columns 11–20.
Elective 6 gives levels of unequal size, and elective 7 changes the
symbols; [Writing decks](decks.md) lists the rest.

## Adding a north arrow and a scale

SYMAP printed no arrow or scale of its own: the deck draws them as legends,
in a C-OTOLEGENDS package placed anywhere before F-MAP. The bottom left of
this map is empty, so the arrow goes there, with the scale beside it.

```text
C-OTOLEGENDS
    1    P      39.0       4.0
*
    3    P      39.0       4.0       1.0      -1.0
***
    5    P      39.0       4.0       2.0      -2.0
*****
    1    P      39.0       4.0       3.0
N
     -   L      45.0      10.0
                45.0      20.0
     I   P      45.0      10.0
     I   P      45.0      15.0
     I   P      45.0      20.0
    1    P      45.0      10.0      -1.0
0
    1    P      45.0      15.0      -1.0
5
    5    P      45.0      20.0      -1.0
10 KM
    6    P      45.0      10.0      -3.0
SCALE:
99999
```

- A **text legend** has its length in columns 4–5 and `P` in column 10, its
  location in columns 11–30, and the text on the next card. Columns 31–40
  and 41–50 move it by whole lines and characters: the arrow's rows are
  one location moved down one, two and three lines, and to the left so that
  each row is centred.
- A **line legend** has its symbol in columns 6–9 and `L` in column 10, then
  a card for each further vertex. The scale bar is a line 10 km long.
- A **symbol legend** has its symbol in columns 6–9 and `P` in column 10.
  The ticks are `I`, at 0, 5 and 10 km.

The legends use the same kilometres as the data. The spacing follows the
manual's Example 1-16: the label, a blank line, the numbers, then the bar.

```text
I    *                . ++++++++++++++++++++++++++++++++++++  ..............    I
I   ***                      ++++++++++++++++++++++++++++++ ................    I
4  *****                            +++++++++++++++++++++++ ................    4
I    N      SCALE:                         ++++++++++++++  .................    I
I                                                 ++++++ ..................     I
I           0      5     10 KM                            .................     I
I           I------I-----I                                       ........1.     I
```

Each legend falls on the nearest character, so the two halves of the bar
can differ by a character.

## Printing

In the browser, Export saves each map as a PNG image at 300 DPI, which
prints at the original size, or as a line printer file (ASA) or plain text.
From the command line, `--asa` writes the line printer file. The PNG
includes the deck unless you turn that off in the Export menu.

## Recipes

### Areas from GeoJSON or TopoJSON

```bash
cargo run -- import wards.geojson --value density > wards.sym
```

Polygons make a conformant map. Each feature becomes a zone, and borders
shared by two zones are simplified together, so the white boundary lines
stay continuous. You should map rates or densities, not counts, since a
large area with a large count looks more important than it is.

### Values for areas as a surface

```bash
cargo run -- import wards.geojson --value density --contour > surface.sym
```

Each zone's value is placed at its centroid as a data point, and the zones
become the outline, so the surface stops at the edge of the data. This
suits values that vary smoothly across zone boundaries more than values
that belong to the zones.

### Nearest point

```bash
cargo run -- import stations.csv --value zone --proximal > nearest.sym
```

Each character takes the value of the nearest point, which divides the map
into the areas closest to each one. `examples/decks/london-zones.sym`
shows fare zones this way.

### A river as a barrier

The importer leaves lines out, so a barrier is written by hand, as a
D-BARRIERS package of vertices: down in columns 11–20, across in 21–30, and
the strength in 31–40 (`-1.` for a barrier that interpolation cannot
cross). Repeating the last vertex ends a barrier.

```text
D-BARRIERS
                12.0       0.0       -1.
                14.5      20.0       -1.
                13.0      42.0       -1.
                13.0      42.0       -1.
99999
```

The vertices are in the deck's own units. The table before the map gives
the conversion: `ROW = DOWN COORDINATE * 0.9871` means one kilometre down
is about one line, so a location can be read from the printed border's
row and column marks and converted back. `examples/decks/barrier.sym` maps
a river with and without its barrier, and
`examples/decks/london-house-prices.sym` uses the Thames.

### Wide areas

`--width` sets the map's width in inches. Maps wider than 13 inches print
in panels, as the line printer did. The Orientation control turns a sheet
sideways, as the Laboratory's report maps were often printed.

### A deck written by hand

A deck needs no importer. Locations can be in any units, as long as
elective 2 gives their extent:

```text
B-DATA POINTS
                 1.0       1.0
                 1.0       9.0
                 9.0       1.0
                 9.0       9.0
                 5.0       5.0
99999
E-VALUES
                10.0
                20.0
                30.0
                40.0
                50.0
99999
F-MAP
FIVE POINTS


    1            5.0       5.0
    2            0.0       0.0      10.0      10.0
99999
999999
```

The two blank cards after the title are the second and third title cards.
The manual's examples in `examples/decks/` show every package in use.
