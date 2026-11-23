#!/usr/bin/env python3
"""Write examples/decks/la-carbon-monoxide.sym from EPA AirData monitor
summaries, after Dangermond's Los Angeles County maps (Harvard, 1969).

Inputs: examples/data/la-co-sites-2013-2024.csv, examples/data/la-county.geojson.
"""

import csv
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITES = ROOT / "examples/data/la-co-sites-2013-2024.csv"
COUNTY = ROOT / "examples/data/la-county.geojson"
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

# Projection: equirectangular about the county's centre, in km. Over 1.3
# degrees of longitude the error is well under a character.
LAT0, LON0 = 34.25, -118.25
KX = 111.32 * math.cos(math.radians(LAT0))
KY = 110.57
# Map frame in longitude and latitude: the county plus a margin.
WLON, ELON, SLAT, NLAT = -118.98, -117.61, 33.68, 34.85


def xy(lat, lon):
    return (lon - LON0) * KX, (lat - LAT0) * KY


TOP = xy(NLAT, 0)[1]
LEFT = xy(0, WLON)[0]


def da(lat, lon):
    """Down and across, in km, from the frame's top left corner."""
    x, y = xy(lat, lon)
    return TOP - y, x - LEFT


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


def text(label, d, a, shift=0.0):
    return [f"{len(label):5d}    P{d:10.2f}{a:10.2f}{shift:10.1f}", label]


def main():
    sites = [r for r in csv.DictReader(SITES.open())
             if int(r["years"]) >= MIN_YEARS
             and (r["county"] == "Los Angeles" or r["site_id"] in ADJOINING)]
    sites.sort(key=lambda r: (r["county"] != "Los Angeles", r["site_id"]))
    county = [r for r in sites if r["county"] == "Los Angeles"]
    ring = json.load(COUNTY.open())["features"][0]["geometry"]["coordinates"][0]

    height, width = da(SLAT, ELON)
    inches = 7.0
    h_in = inches * height / width

    o = []
    w = o.append
    w(HEADER.format(n=len(sites), nc=len(county), na=len(sites) - len(county),
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
    for label, lat, lon in [
        ("KERN COUNTY", 34.835, -118.40),
        ("VENTURA", 34.62, -118.85),
        ("COUNTY", 34.60, -118.85),
        ("SAN BERNARDINO", 34.70, -117.88),
        ("COUNTY", 34.68, -117.88),
        ("ORANGE COUNTY", 33.76, -117.86),
        ("PACIFIC OCEAN", 33.80, -118.70),
        ("DESERT", 34.62, -118.30),
        ("SAN GABRIEL MOUNTAINS", 34.31, -118.11),
        ("NEWHALL PASS", 34.335, -118.62),
    ]:
        o += text(label, *da(lat, lon))
    w("99999")
    w("E-VALUES")
    o += [f"{'':10}{float(r['max8h']):10.1f}" for r in sites]
    o += [f"{'':10}{0.0:10.1f}" for _ in PEAKS]
    w("99999")
    w("F-MAP")
    w("LOS ANGELES COUNTY - MAXIMUM CARBON MONOXIDE CONCENTRATION")
    w("HIGHEST 8-HOUR MEAN, PPM, 2013-2024 (AFTER DANGERMOND, HARVARD, 1969)")
    w("A. BARRIERS AND MOUNTAIN BACKGROUND POINTS")
    w(f"    1{'':6}{h_in:10.2f}{inches:10.2f}")
    w(f"    2{'':6}{0:10.2f}{0:10.2f}{height:10.2f}{width:10.2f}")
    w("    3            10.")
    w("    4           0.00")
    w("    5           5.00")
    w("   10")
    w(f"{len(sites)} MONITORS ({len(county)} IN THE COUNTY), 9 MOUNTAIN PEAKS AT 0.0 PPM,")
    w("TWO PERMEABLE BARRIERS OPEN AT NEWHALL PASS.  LEVELS EVERY 0.5 PPM.")
    w("9999")
    w("99999")
    # Map b: the measured data points only, with no barriers.
    w("B-DATA POINTS")
    o += [card(*da(float(r["latitude"]), float(r["longitude"]))) for r in sites]
    w("99999")
    w("D-BARRIERS")
    w("99999")
    w("E-VALUES")
    o += [f"{'':10}{float(r['max8h']):10.1f}" for r in sites]
    w("99999")
    w("F-MAP")
    w("LOS ANGELES COUNTY - MAXIMUM CARBON MONOXIDE CONCENTRATION")
    w("HIGHEST 8-HOUR MEAN, PPM, 2013-2024 (AFTER DANGERMOND, HARVARD, 1969)")
    w("B. MEASURED DATA POINTS ONLY")
    w("   12")
    w("   10")
    w(f"{len(sites)} MONITORS ({len(county)} IN THE COUNTY), NO BACKGROUND POINTS,")
    w("NO BARRIERS.  LEVELS EVERY 0.5 PPM.")
    w("9999")
    w("99999")
    w("999999")
    OUT.write_text("\n".join(o) + "\n")
    print(f"{OUT.relative_to(ROOT)}: {len(sites)} stations, {len(PEAKS)} peaks, "
          f"frame {height:.1f} x {width:.1f} km", file=sys.stderr)


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
# Coordinates are km, down and across from 34.85 N, 118.98 W, on an
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
