#!/usr/bin/env python3
"""Write examples/decks/la-carbon-monoxide.sym from EPA AirData monitor
summaries, after Dangermond's Los Angeles County maps (Harvard, 1969).

Inputs: examples/data/la-co-sites-2013-2024.csv, examples/data/la-county.geojson
and examples/data/la-neighbour-lines.geojson.
"""

import csv
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITES = ROOT / "examples/data/la-co-sites-2013-2024.csv"
COUNTY = ROOT / "examples/data/la-county.geojson"
NEIGHBOURS = ROOT / "examples/data/la-neighbour-lines.geojson"
OUT = ROOT / "examples/decks/la-carbon-monoxide.sym"

# Adjoining-county stations, as Dangermond used 8 to the east and west.
# Ventura County had no carbon monoxide monitor in 2013-2024.
ADJOINING = {
    "06-059-0007", "06-059-0008", "06-059-5001", "06-059-1003",  # Orange
    "06-071-1004", "06-071-0026", "06-071-2002",  # San Bernardino
    "06-065-8005",  # Riverside
}
MIN_YEARS = 5

# Background points of 0.0 ppm on nine peaks above 5,000 feet, as on Figure
# I-1a. Summits from USGS GNIS, to about 0.01 degree.
PEAKS = [
    ("MOUNT SAN ANTONIO", 34.289, -117.646),
    ("MOUNT BADEN-POWELL", 34.358, -117.765),
    ("MOUNT ISLIP", 34.243, -117.852),
    ("WATERMAN MOUNTAIN", 34.334, -117.943),
    ("MOUNT PACIFICO", 34.381, -118.035),
    ("STRAWBERRY PEAK", 34.291, -118.119),
    ("MOUNT GLEASON", 34.376, -118.178),
    ("MOUNT LUKENS", 34.269, -118.239),
    ("BURNT PEAK", 34.682, -118.578),
]

# Two protruding barriers meet at Newhall Pass, where their strength is 0;
# from there each runs to the map border, west along the Santa Susana
# Mountains and east along the front of the San Gabriel Mountains. As in
# Report I, the strength at each vertex is twice its distance from the pass
# along the barrier, in the deck's units (km).
PASS = (34.330, -118.510)
WEST = [PASS, (34.318, -118.620), (34.340, -118.740), (34.370, -118.880), (34.400, -119.050)]
EAST = [PASS, (34.300, -118.400), (34.265, -118.280), (34.215, -118.170),
        (34.195, -118.040), (34.180, -117.900), (34.185, -117.760), (34.210, -117.540)]

# Projection: equirectangular about the county's centre, in km. Over 1.7
# degrees of longitude the error is well under a character.
LAT0, LON0 = 34.25, -118.25
KX = 111.32 * math.cos(math.radians(LAT0))
KY = 110.57
# Map frame in longitude and latitude, measured off the scan of Figure I-1:
# Kern County to the north, the Pacific to the south, with room for the
# north arrow, scale and lettered title below the coast.
WLON, ELON, SLAT, NLAT = -119.065, -117.382, 33.392, 34.916
# Printed width, as on the original: about 224 characters (22.4 inches),
# printed in two panels. Characters are 0.1 inch wide and 0.125 inch tall.
WIDTH_IN = 22.4

# Furniture positions are taken from the scan (400 dpi, Figure I-1a), in
# its pixels, where the map border runs from (50, 65) to (1805, 2110).
SCAN_FRAME = (50, 65, 1805, 2110)


def xy(lat, lon):
    return (lon - LON0) * KX, (lat - LAT0) * KY


TOP = xy(NLAT, 0)[1]
LEFT = xy(0, WLON)[0]
HEIGHT_KM = TOP - xy(SLAT, 0)[1]
WIDTH_KM = xy(0, ELON)[0] - LEFT
SCALE = WIDTH_IN / WIDTH_KM  # inches per km
COL = 0.1 / SCALE  # one character across, in km
ROW = 0.125 / SCALE  # one line down, in km


def da(lat, lon):
    """Down and across, in km, from the frame's top left corner."""
    x, y = xy(lat, lon)
    return TOP - y, x - LEFT


def scan(x, y):
    """Down and across, in km, of a point on the scan."""
    x0, y0, x1, y1 = SCAN_FRAME
    return (y - y0) / (y1 - y0) * HEIGHT_KM, (x - x0) / (x1 - x0) * WIDTH_KM


def card(d, a, extra=""):
    return f"{'':10}{d:10.2f}{a:10.2f}{extra}"


def barrier(pts):
    out, dist = [], 0.0
    prev = None
    for lat, lon in pts:
        d, a = da(lat, lon)
        if prev:
            dist += math.hypot(d - prev[0], a - prev[1])
        out.append(card(d, a, f"{2 * dist:10.1f}"))
        prev = (d, a)
    out.append(out[-1])  # repeating the last vertex ends the barrier
    return out


def text(label, d, a):
    """A text legend centred on (d, a)."""
    return [f"{len(label):5d}    P{d:10.2f}{a:10.2f}", label]


def line(symbol, pts):
    out = [f"{'':5}{symbol:<4}L{pts[0][0]:10.2f}{pts[0][1]:10.2f}"]
    return out + [card(d, a) for d, a in pts[1:]]


def area(symbol, pts):
    pts = list(pts) + [pts[0]]
    out = [f"{'':5}{symbol:<4}A{pts[0][0]:10.2f}{pts[0][1]:10.2f}"]
    return out + [card(d, a) for d, a in pts[1:]]


def spaced(label):
    return " ".join(label)


def underlined(label, d, a):
    """Spaced capitals with a rule beneath, as the original's county names."""
    t = spaced(label)
    half = len(t) / 2 * COL
    return text(t, d, a) + line("-", [(d + ROW, a - half), (d + ROW, a + half)])


def knockout(label, d, a):
    """Black text on a strip cleared from the map, centred on (d, a)."""
    half = (len(label) / 2 + 1) * COL
    strip = [(d - 0.5 * ROW, a - half), (d - 0.5 * ROW, a + half),
             (d + 0.5 * ROW, a + half), (d + 0.5 * ROW, a - half)]
    return area("", strip) + text(label, d, a)


# Block capitals 6 characters wide and 5 lines tall, drawn as line legends:
# continuous strokes of overprinted O and X, as on the original's title.
LETTERS = {
    "L": [[(0, 0), (0, 4)], [(0, 4), (5, 4)]],
    "O": [[(0, 0), (5, 0)], [(0, 4), (5, 4)], [(0, 0), (0, 4)], [(5, 0), (5, 4)]],
    "S": [[(0, 0), (5, 0)], [(0, 2), (5, 2)], [(0, 4), (5, 4)], [(0, 0), (0, 2)], [(5, 2), (5, 4)]],
    "A": [[(0, 0), (5, 0)], [(0, 2), (5, 2)], [(0, 0), (0, 4)], [(5, 0), (5, 4)]],
    "N": [[(0, 0), (0, 4)], [(5, 0), (5, 4)], [(1, 1), (4, 3)]],
    "G": [[(0, 0), (5, 0)], [(3, 2), (5, 2)], [(0, 4), (5, 4)], [(0, 0), (0, 4)], [(5, 2), (5, 4)]],
    "E": [[(0, 0), (5, 0)], [(0, 2), (4, 2)], [(0, 4), (5, 4)], [(0, 0), (0, 4)]],
    "C": [[(0, 0), (5, 0)], [(0, 4), (5, 4)], [(0, 0), (0, 4)]],
    "U": [[(0, 4), (5, 4)], [(0, 0), (0, 4)], [(5, 0), (5, 4)]],
    "T": [[(0, 0), (5, 0)], [(3, 0), (3, 4)]],
    "Y": [[(0, 0), (2, 2)], [(5, 0), (3, 2)], [(3, 2), (3, 4)]],
}


def lettering(word, d, a, pitch=8):
    """Block capitals with their top left corner at (d, a)."""
    out = []
    for k, ch in enumerate(word):
        for (c0, r0), (c1, r1) in LETTERS.get(ch, []):
            sym = "OX"  # dense continuous strokes, as on the original
            pts = [(d + r * ROW, a + (k * pitch + c) * COL) for c, r in ((c0, r0), (c1, r1))]
            out += line(sym, pts)
    return out


def north_arrow(d, a):
    """A solid triangle 8 lines tall with its apex at (d, a), and NORTH."""
    tri = [(d, a), (d + 7 * ROW, a + 7.5 * COL), (d + 7 * ROW, a - 7.5 * COL)]
    return area("OXAV", tri) + text(spaced("NORTH"), d + 10 * ROW, a)


def scale_bar(d, a, segments=4, miles=5):
    """Segments of five statute miles, alternately on two lines, with ticks
    and figures, as on the original."""
    km = miles * 1.609344
    out = []
    for k in range(segments + 1):
        x = a + k * km
        out += line("I", [(d - ROW, x), (d + ROW, x)])
        out += text(str(k * miles), d - 2 * ROW, x)
        if k < segments:
            dd = d if k % 2 == 0 else d + ROW
            out += line("-", [(dd, x + COL), (dd, x + km - COL)])
    out += text(spaced("SCALE IN STATUTE MILES"), d + 3 * ROW, a + segments * km / 2)
    return out


def neighbour_lines():
    out = []
    for f in json.load(NEIGHBOURS.open())["features"]:
        for part in f["geometry"]["coordinates"]:
            out += line(".", [da(lat, lon) for lon, lat in part])
    return out


def short_name(name):
    n = name.upper().split("(")[0].split(" #")[0].strip()
    return {"LOS ANGELES-NORTH MAIN STREET": "LOS ANGELES", "LAX HASTINGS": "LAX",
            "LANCASTER-DIVISION STREET": "LANCASTER"}.get(n, n)


def furniture(county):
    o = neighbour_lines()
    # County names and the national forest, spaced and placed as on the scan.
    for label, x, y in [("KERN COUNTY", 788, 127), ("VENTURA COUNTY", 278, 843),
                        ("ORANGE COUNTY", 1453, 1616)]:
        o += underlined(label, *scan(x, y))
    for x, y in [(595, 390), (1368, 870)]:
        o += knockout(spaced("ANGELES NATIONAL FOREST"), *scan(x, y))
    # Coastal places, in the sea.
    for label, x, y in [("SANTA MONICA", 575, 1296), ("MANHATTAN BEACH", 640, 1422),
                        ("REDONDO BEACH", 680, 1531), ("SAN PEDRO BAY", 1013, 1651)]:
        o += text(label, *scan(x, y))
    # Each station in the county named on a cleared strip just to its right,
    # leaving its level number visible.
    for r in county:
        n = short_name(r["local_site_name"])
        d, a = da(float(r["latitude"]), float(r["longitude"]))
        o += knockout(n, d, a + (len(n) / 2 + 2) * COL)
    o += north_arrow(*scan(355, 1610))
    o += scale_bar(*scan(210, 1872))
    d, a = scan(140, 2040)
    o += lettering("LOS ANGELES COUNTY", d, a)
    d, a = scan(1460, 2035)
    for k, t in enumerate(CREDITS):
        o += text(t, d + (k + k // 2) * ROW, a)
    return o


CREDITS = [
    "REBUILT IN A SYMAP EMULATOR, 2026, AFTER",
    "P. J. DANGERMOND, HARVARD LCGSA, 1969",
    "CARBON MONOXIDE: US EPA AIRDATA 2013-2024",
    "BOUNDARIES: US CENSUS BUREAU, CB 2023",
]


def station_list(rows):
    """Two columns of names, as in the original's legend."""
    names = [r["local_site_name"].upper() for r in rows]
    half = (len(names) + 1) // 2
    return [f"     {a:<34}{b}".rstrip() for a, b in
            zip(names[:half], names[half:] + [""])]


def main():
    sites = [r for r in csv.DictReader(SITES.open())
             if int(r["years"]) >= MIN_YEARS
             and (r["county"] == "Los Angeles" or r["site_id"] in ADJOINING)]
    sites.sort(key=lambda r: (r["county"] != "Los Angeles", r["site_id"]))
    county = [r for r in sites if r["county"] == "Los Angeles"]
    outside = [r for r in sites if r["county"] != "Los Angeles"]
    ring = json.load(COUNTY.open())["features"][0]["geometry"]["coordinates"][0]
    h_in = WIDTH_IN * HEIGHT_KM / WIDTH_KM

    o = []
    w = o.append
    w(HEADER.format(n=len(sites), nc=len(county), na=len(outside),
                    stations="\n".join(
                        f"#   {r['site_id']}  {r['local_site_name']:<32}{r['county']:<16}"
                        f"{r['years']:>3} yrs  {float(r['max8h']):4.1f}" for r in sites)))
    w("A-OUTLINE")
    o += [card(*da(lat, lon)) for lon, lat in ring]
    w("99999")
    w("B-DATA POINTS")
    o += [card(*da(float(r["latitude"]), float(r["longitude"]))) for r in sites]
    o += [card(*da(lat, lon)) for _, lat, lon in PEAKS]
    w("99999")
    w("D-BARRIERS")
    o += barrier(WEST) + barrier(EAST)
    w("99999")
    w("C-OTOLEGENDS")
    o += furniture(county)
    w("99999")
    w("E-VALUES")
    o += [f"{'':10}{float(r['max8h']):10.1f}" for r in sites]
    o += [f"{'':10}{0.0:10.1f}" for _ in PEAKS]
    w("99999")

    def fmap(sub, first):
        w("F-MAP")
        w("MAXIMUM CARBON MONOXIDE CONCENTRATION MEASURED IN P.P.M.")
        w("HIGHEST 8-HOUR MEAN, 2013-2024 (AFTER DANGERMOND, HARVARD, 1969)")
        w(sub)
        if first:
            w(f"    1{'':6}{h_in:10.2f}{WIDTH_IN:10.2f}")
            w(f"    2{'':6}{0:10.2f}{0:10.2f}{HEIGHT_KM:10.2f}{WIDTH_KM:10.2f}")
            w("    3            10.")
            w("    4           0.00")
            w("    5           5.00")
            w("    8")
        else:
            w("   12")
        w("   10")
        w("THE FOLLOWING IS A LIST OF STATIONS USED WITHIN THE COUNTY")
        o.extend(station_list(county))
        w("THE FOLLOWING IS A LIST OF STATIONS OUTSIDE LOS ANGELES COUNTY")
        o.extend(station_list(outside))
        w("9999")
        w("99999")

    fmap("A. BARRIERS AND MOUNTAIN BACKGROUND POINTS", True)
    # Map b: the measured data points only, with no barriers.
    w("B-DATA POINTS")
    o += [card(*da(float(r["latitude"]), float(r["longitude"]))) for r in sites]
    w("99999")
    w("D-BARRIERS")
    w("99999")
    w("E-VALUES")
    o += [f"{'':10}{float(r['max8h']):10.1f}" for r in sites]
    w("99999")
    fmap("B. MEASURED DATA POINTS ONLY", False)
    w("999999")
    OUT.write_text("\n".join(o) + "\n")
    print(f"{OUT.relative_to(ROOT)}: {len(sites)} stations, {len(PEAKS)} peaks, "
          f"frame {HEIGHT_KM:.1f} x {WIDTH_KM:.1f} km, {h_in:.2f} x {WIDTH_IN} in",
          file=sys.stderr)


HEADER = """\
# Los Angeles County: maximum carbon monoxide concentration, after Figure
# I-1 of P. J. Dangermond, 'Computer Mapping as an Aid in Air Pollution
# Studies, Volume Two: Individual Reports, Report I: California Study'
# (Harvard Laboratory for Computer Graphics and Spatial Analysis, April
# 1970; NTIS PB 204 194, APTD-0784), made in 1969 for the National Air
# Pollution Control Administration under grant 68A-2405D. Its two maps
# show the calculated annual maximum CO concentration from 8-hour values,
# 1956-1967, at 30 stations (22 in the county's valleys, 8 in adjoining
# counties): (a) with two variable-strength protruding permeable barriers
# open at a mountain pass and 0.0 ppm background points on 9 peaks above
# 5,000 feet, and (b) with the measured data points only, where SYMAP
# carries the valley's pollution over the mountains into the desert.
# Generated by scripts/la_co_deck.py.
#
# Here the value is the highest '1st Max Value' of the 8-hour running mean
# of carbon monoxide (parameter 42101, '8-HR RUN AVG END HOUR'), ppm, at
# each monitor over the twelve years 2013-2024, from the US EPA's AirData
# annual summaries (annual_conc_by_monitor_YYYY.zip,
# https://aqs.epa.gov/aqsweb/airdata/download_files.html), US Government
# work in the public domain. Monitors with fewer than {MIN_YEARS} years of data
# are left out. {nc} monitors are in the county and {na} in Orange, San
# Bernardino and Riverside counties; Ventura County had none. The
# original's 1956-1967 maxima were several times higher.
# examples/data/la-co-sites-2013-2024.csv lists every monitor.
#
# Site, name, county, years of data, highest 8-hour mean (ppm):
{stations}
#
# The outline is Los Angeles County without its islands, from the US
# Census Bureau's cartographic boundary file cb_2023_us_county_500k
# (examples/data/la-county.geojson), public domain. Background points
# and barriers follow Report I's description; their exact positions on
# the original are not legible, so they are placed on the San Gabriel and
# Santa Susana mountains from USGS summits. Barrier strength is twice the
# distance from Newhall Pass along the barrier, Report I's rule of thumb.
# Coordinates are km, down and across from 34.916 N, 119.065 W, on an
# equirectangular projection about 34.25 N, 118.25 W.
#
# References:
#   Dangermond, P. J. (1970) Report I, California Study, as above.
#   Larsen, R. I. (1969) A new mathematical model of air pollutant
#     concentration averaging time and frequency. Journal of the Air
#     Pollution Control Association 19(1), 24-30.
#     doi:10.1080/00022470.1969.10466453 (the model behind the original's
#     'calculated' maxima).
#   Shepard, D. (1968) A two-dimensional interpolation function for
#     irregularly-spaced data. Proceedings of the 1968 23rd ACM National
#     Conference, 517-524. doi:10.1145/800186.810616 (SYMAP's
#     interpolation and barriers).
#   Chrisman, N. (2006) Charting the Unknown: How Computer Mapping at
#     Harvard Became GIS. ESRI Press, Redlands.
""".replace("{MIN_YEARS}", str(MIN_YEARS))

if __name__ == "__main__":
    main()
