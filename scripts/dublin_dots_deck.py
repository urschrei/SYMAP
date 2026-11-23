"""Write examples/decks/dublin-dots.sym and examples/data/dublin-ed-population.csv.

A dot map (elective 28) of County Dublin's usual residents by Electoral
Division, Census 2022: one dot for every 1,000 residents, scattered across
each division.

Inputs, fetched from the CSO Census Hub and the EPA (see the deck header):

    python3 scripts/dublin_dots_deck.py ed-2022.geojson liffey.geojson

ed-2022.geojson: CensusHub2022_T1_2_ED layer 0, where COUNTY='DUBLIN',
outSR=2157. liffey.geojson: EPA:WATER_RIVNETROUTES where EPA_Name is
Liffey, srsName=EPSG:2157. Needs shapely 2.1 or later.
"""

import csv
import json
import sys
import textwrap
from pathlib import Path

import shapely
from shapely.geometry import LineString, Polygon, shape
from shapely.ops import linemerge, unary_union

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "examples/decks/dublin-dots.sym"
CSV = ROOT / "examples/data/dublin-ed-population.csv"

# Irish Transverse Mercator kilometres: down from northing N0, across from
# easting E0, at 0.35 inch per km.
N0, E0 = 767.0, 696.5
INCH_PER_KM = 0.35
PER_DOT = 1000
SIMPLIFY_M = 100
MIN_PART_KM2 = 0.05
HEIGHT_KM, WIDTH_KM = 51.5, 36.7

LA_SHORT = {
    "DUBLIN CITY COUNCIL": "Dublin City",
    "DUN LAOGHAIRE-RATHDOWN COUNTY COUNCIL": "Dun Laoghaire-Rathdown",
    "FINGAL COUNTY COUNCIL": "Fingal",
    "SOUTH DUBLIN COUNTY COUNCIL": "South Dublin",
}

HEADER = """\
# County Dublin: a dot map of usual residents by Electoral Division,
# Census 2022 (elective 28), one dot for every 1,000 residents.
#
# Values: usually resident population (T1_2T) of the 322 CSO Electoral
#   Divisions in the four Dublin local authorities (1,458,154 in all).
#   'Census 2022 - Table 1.2 - Population by sex and marital status by
#   Electoral Divisions', Central Statistics Office (CSO) Census Hub,
#   with boundaries 'CSO Electoral Divisions 2022' (20 m generalised,
#   from Tailte Eireann). https://services-eu1.arcgis.com/BuS9rtTsYEV5C0xh/
#   arcgis/rest/services/CensusHub2022_T1_2_ED/FeatureServer
#   Source: CSO / Tailte Eireann, licensed under Creative Commons
#   Attribution 4.0 (CC BY 4.0). https://creativecommons.org/licenses/by/4.0/
# River: River Liffey from the EPA river network 'Flow Network
#   (Indicative)', layer EPA:WATER_RIVNETROUTES (1:50,000), Environmental
#   Protection Agency, via https://gis.epa.ie/geoserver/ows (WFS).
#   Licensed under CC BY 4.0.
# Changes: boundaries are simplified together (shapely coverage_simplify,
# {simplify} m) so neighbours keep shared edges, and parts under
# {min_part} sq km are left out; holes are dropped and zones are listed
# smallest first, so a division inside another keeps its own characters.
# The county outline is the union of the divisions. The Liffey is clipped
# to the county and simplified to 50 m.
# A division's dots are placed at random among its characters, so dots
# show how many people live in a division, not where in it they live.
{scale_note}
# Coordinates are Irish Transverse Mercator kilometres: down from
# northing {n0:.0f} km, across from easting {e0:.1f} km; {inch} inch per km.
# Zone references follow ED_ID_STR order, as do the values.
"""


def km(x, y):
    return (N0 - y / 1000.0, x / 1000.0 - E0)


def ring_cards(ref, sym, coords):
    lines = []
    for k, (x, y) in enumerate(coords):
        d, a = km(x, y)
        if k == 0:
            lines.append(f"{ref:>5}    {sym}{d:10.2f}{a:10.2f}")
        else:
            lines.append(f"{'':10}{d:10.2f}{a:10.2f}")
    return lines


def legend_line(sym, line):
    out = []
    for k, (x, y) in enumerate(line.coords):
        d, a = km(x, y)
        head = f"{sym:>6}   L" if k == 0 else " " * 10
        out.append(f"{head}{d:10.2f}{a:10.2f}")
    return out


def text(down, across, words, dr=None, dc=None):
    off = "" if dr is None else f"{dr:10.2f}{dc:10.2f}"
    return [f"{len(words):>5}    P{down:10.2f}{across:10.2f}{off}", words]


def point(sym, down, across, dr, dc):
    return f"{sym:>6}   P{down:10.2f}{across:10.2f}{dr:10.2f}{dc:10.2f}"


def scale_note(lost, full, cells):
    """Header lines on what the character grid does to the dot counts."""
    pop = sum(r["population"] for r in lost)
    if not lost:
        a = "every division covers at least one character"
    elif len(lost) == 1:
        a = f"one small division ({pop:,} residents) covers no character and gets no dots"
    else:
        a = f"{len(lost)} small divisions ({pop:,} residents) cover no character and get no dots"
    cut = sum(n - cells[r] for r, n in full)
    b = (
        f"{len(full)} crowded divisions have more dots than characters, so every "
        f"character there is a dot ({cut:,} dots short)"
    )
    text = f"At this scale {a}, and {b}."
    return "\n".join("# " + l for l in textwrap.wrap(text, 70))


def main(ed_path, liffey_path):
    feats = json.loads(Path(ed_path).read_text())["features"]
    feats.sort(key=lambda f: f["properties"]["ED_ID_STR"])
    geoms = [shape(f["geometry"]) for f in feats]
    simple = shapely.coverage_simplify(geoms, SIMPLIFY_M)

    zones = []  # (area, ref, ring coords)
    rows = []
    for ref, (f, g, s) in enumerate(zip(feats, geoms, simple), start=1):
        p = f["properties"]
        parts = [s] if isinstance(s, Polygon) else list(s.geoms)
        parts = [q for q in parts if q.area / 1e6 >= MIN_PART_KM2]
        for q in parts:
            zones.append((q.area, ref, list(q.exterior.coords)))
        rows.append(
            {
                "ref": ref,
                "ed_id": p["ED_ID_STR"],
                "name": p["ED_ENGLISH"].title(),
                "local_authority": LA_SHORT[p["LOCAL_AUTHORITY"]],
                "population": int(p["T1_2T"]),
                "area_km2": round(g.area / 1e6, 3),
            }
        )
    zones.sort(key=lambda z: z[0])

    county = unary_union(geoms).buffer(0)
    outline = shapely.coverage_simplify([county], SIMPLIFY_M)[0]
    outline_parts = [outline] if isinstance(outline, Polygon) else list(outline.geoms)
    outline_parts = [q for q in outline_parts if q.area / 1e6 >= MIN_PART_KM2]

    rivers = [
        shape(f["geometry"])
        for f in json.loads(Path(liffey_path).read_text())["features"]
    ]
    liffey = linemerge(unary_union([shapely.force_2d(r) for r in rivers]))
    liffey = liffey.intersection(county.buffer(200)).simplify(50)
    liffey_lines = [liffey] if isinstance(liffey, LineString) else list(liffey.geoms)
    liffey_lines = [l for l in liffey_lines if l.length > 500]

    # Divisions whose outlines hold no character centre get no dots.
    lpk, cpk = 8 * INCH_PER_KM, 10 * INCH_PER_KM
    centres = shapely.points(
        [
            (E0 * 1000 + c / cpk * 1000, (N0 - r / lpk) * 1000)
            for r in range(1, round(HEIGHT_KM * lpk))
            for c in range(1, round(WIDTH_KM * cpk))
        ]
    )
    cells = {}
    for _, ref, coords in zones:
        cells[ref] = cells.get(ref, 0) + int(
            shapely.contains_xy(
                Polygon(coords), *shapely.get_coordinates(centres).T
            ).sum()
        )
    lost = [r for r in rows if cells.get(r["ref"], 0) == 0]
    dots = {r["ref"]: round(r["population"] / PER_DOT) for r in rows}
    full = [(r, n) for r, n in dots.items() if 0 < cells.get(r, 0) < n]

    with open(CSV, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)

    cards = []
    cards.append("A-CONFORMOLINES")
    for _, ref, coords in zones:
        cards += ring_cards(ref, "A", coords)
    cards.append("99999")

    cards.append("C-OTOLEGENDS")
    for q in outline_parts:
        cards += legend_line("+", q.exterior)
    for l in liffey_lines:
        cards += legend_line("~", l)
    cards += text(*km(716000, 734500), "LIFFEY", -1.0, 0.0)
    cards += text(*km(726500, 734500), "DUBLIN BAY")
    cards += text(*km(727500, 746500), "IRISH SEA")
    # Scale bar: 5 km in the bottom left, outside the county.
    d0, a0 = 48.5, 2.0
    cards += [f"{'=':>6}   L{d0:10.2f}{a0:10.2f}", f"{'':10}{d0:10.2f}{a0 + 5:10.2f}"]
    cards += text(d0, a0, "SCALE", -2.0, 0.0)
    for k, lab in ((0, "0"), (5, "5 KM")):
        cards.append(f"{'I':>6}   P{d0:10.2f}{a0 + k:10.2f}")
        cards += text(d0, a0 + k, lab, 1.0, 0.0)
    cards.append("99999")

    cards.append("E-VALUES")
    cards += [f"{r['population']:20.1f}" for r in rows]
    cards.append("99999")

    hi = max(r["population"] for r in rows)
    height = HEIGHT_KM * INCH_PER_KM
    width = WIDTH_KM * INCH_PER_KM
    cards += [
        "F-MAP",
        "COUNTY DUBLIN - WHERE PEOPLE LIVE, CENSUS 2022",
        "ONE DOT FOR EVERY 1,000 USUAL RESIDENTS",
        "DOT MAP OF 322 ELECTORAL DIVISIONS",
        f"    1{height:15.2f}{width:10.2f}",
        f"    2{0:15.2f}{0:10.2f}{HEIGHT_KM:10.2f}{WIDTH_KM:10.2f}",
        f"    3{1:14d}.",
        f"    4{0:15.2f}",
        f"    5{hi:15.2f}",
        "    8",
        "    9",
        f"   28{'':15}{PER_DOT:10.2f}",
        "   10",
        "EACH DOT IS 1,000 PEOPLE, PLACED AT RANDOM WITHIN THEIR ELECTORAL",
        "DIVISION. + IS THE COUNTY BOUNDARY AND COAST, ~ THE RIVER LIFFEY.",
        "9999",
        "99999",
        "999999",
    ]

    header = HEADER.format(
        simplify=SIMPLIFY_M,
        min_part=MIN_PART_KM2,
        scale_note=scale_note(lost, full, cells),
        n0=N0,
        e0=E0,
        inch=INCH_PER_KM,
    )
    OUT.write_text(header + "\n".join(cards) + "\n")
    print(f"wrote {OUT} ({len(zones)} outlines) and {CSV} ({len(rows)} rows)")


if __name__ == "__main__":
    main(*sys.argv[1:3])
