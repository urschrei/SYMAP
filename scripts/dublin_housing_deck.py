"""Write examples/decks/dublin-housing.sym from examples/data/dublin-ed-dwellings.csv.

The same dwelling densities printed twice: as a conformant map of Dublin
City's 162 electoral divisions, and as a contour map forced from the same
zones with elective 27, each zone's value placed at its centre. Zone
outlines, the Liffey and the map furniture are taken from dublin-density.sym;
the city outline for the contour map is the edges the zones do not share.
Both maps print central Dublin at twice the scale of dublin-density.sym.

    python3 scripts/dublin_housing_deck.py
"""

import csv
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "examples/data/dublin-ed-dwellings.csv"
ZONES = ROOT / "examples/decks/dublin-density.sym"
OUT = ROOT / "examples/decks/dublin-housing.sym"

INTERVALS = [1000, 1000, 1000, 1000, 2000, 4000]  # 0-1k-2k-3k-4k-6k-10k
VMAX = 10000.0

# The window printed: central Dublin, kilometres down and across in the
# coordinates of dublin-density.sym, at twice its scale (1.2 inch per km),
# so a median ED (0.57 km2) spans about 60 characters rather than 16.
# Zones and data points outside the window still shape the contours.
DOWN, ACROSS = (3.5, 10.5), (3.0, 13.8)
SCALE = 1.2
# The north arrow and scale inset: 12 lines by 38 columns, two lines and
# three columns in from the window's south-east corner, which is mostly
# Sandymount Strand and the bay.
INSET_H, INSET_W = 12 / 8 / SCALE, 38 / 10 / SCALE
INSET = (DOWN[1] - INSET_H - 2 / 8 / SCALE, ACROSS[1] - INSET_W - 3 / 10 / SCALE)


def legends(cards):
    """The Liffey and its labels from dublin-density.sym, with DUBLIN BAY
    moved into the window and a new north arrow and scale inset."""
    out = []
    for c in cards:
        if c[10:20].strip() == "10.30" and c[9] == "A":
            break  # the old inset starts here
        out.append(c)
    k = out.index("DUBLIN BAY")
    out[k - 1] = f"{10:5d}    P{6.60:10.2f}{12.80:10.2f}"
    d0, a0 = INSET
    d1, a1 = d0 + INSET_H, a0 + INSET_W
    f = lambda d, a: f"{d:10.2f}{a:10.2f}"
    out.append("         A" + f(d0, a0))
    out += [" " * 10 + f(*p) for p in [(d1, a0), (d1, a1), (d0, a1), (d0, a0)]]
    # Rules: dashes along the top and bottom, I down the sides.
    out += [f"     -   L{f(d0, a0)}", " " * 10 + f(d0, a1)]
    out += [f"     -   L{f(d1, a0)}", " " * 10 + f(d1, a1)]
    out += [f"     I   L{f(d0, a0)}", " " * 10 + f(d1, a0)]
    out += [f"     I   L{f(d0, a1)}", " " * 10 + f(d1, a1)]
    for r, c in [(2, 4), (3, 3), (3, 4), (3, 5), (4, 2), (4, 3), (4, 4), (4, 5), (4, 6)]:
        out.append(f"     *   P{f(d0, a0)}{r:10.2f}{c:10.2f}")
    out.append(f"     N   P{f(d0, a0)}{5.0:10.2f}{4.0:10.2f}")
    bar = d0 + 10 / 8 / SCALE  # the bar on the inset's eleventh line
    b0 = a0 + 8 / 10 / SCALE
    out += [f"     =   L{f(bar, b0)}", " " * 10 + f(bar, b0 + 2)]
    out += [f"    5    P{f(bar, b0)}{-3.0:10.2f}{0.0:10.2f}", "SCALE"]
    for km, label in [(0, "0"), (1, "1"), (2, "2 KM")]:
        out.append(f"     I   P{f(bar, b0 + km)}")
        out += [f"{len(label):5d}    P{f(bar, b0 + km)}{-1.0:10.2f}{0.0:10.2f}", label]
    return out


def packages(path):
    """Map each package header to its cards in a deck."""
    out, name, cards = {}, None, []
    for line in path.read_text().splitlines():
        if name is None:
            if line[:1].isalpha():
                name, cards = line.strip(), []
        elif line.startswith("99999"):
            out.setdefault(name, cards)
            name = None
        else:
            cards.append(line)
    return out


def zones(cards):
    """Each zone's reference and ring of (down, across) vertices."""
    out = []
    for c in cards:
        if c[:10].strip():
            out.append((int(c[:5]), []))
        out[-1][1].append((float(c[10:20]), float(c[20:30])))
    return out


def area_centroid(ring):
    a = cd = ca = 0.0
    for (pd, pa), (qd, qa) in zip(ring, ring[1:]):
        x = pa * qd - qa * pd
        a += x
        cd += (pd + qd) * x
        ca += (pa + qa) * x
    return cd / (3 * a), ca / (3 * a)


def inside(pt, ring):
    d, a = pt
    hit = False
    for (pd, pa), (qd, qa) in zip(ring, ring[1:]):
        if (pd > d) != (qd > d) and a < pa + (d - pd) * (qa - pa) / (qd - pd):
            hit = not hit
    return hit


def centre(ring):
    """The area centroid, or for a zone that does not contain it, the
    midpoint of the widest span across the row through the centroid."""
    c = area_centroid(ring)
    if inside(c, ring):
        return c
    d = c[0]
    xs = sorted(
        pa + (d - pd) * (qa - pa) / (qd - pd)
        for (pd, pa), (qd, qa) in zip(ring, ring[1:])
        if (pd > d) != (qd > d)
    )
    spans = list(zip(xs[::2], xs[1::2]))
    lo, hi = max(spans, key=lambda s: s[1] - s[0])
    return d, (lo + hi) / 2


def outline(rings):
    """Rings of the edges that only one zone uses."""
    key = lambda p, q: (p, q) if p <= q else (q, p)
    use = Counter(key(p, q) for r in rings for p, q in zip(r, r[1:]) if p != q)
    edges = [(p, q) for r in rings for p, q in zip(r, r[1:]) if p != q and use[key(p, q)] == 1]
    nxt = {}
    for p, q in edges:
        nxt.setdefault(p, []).append(q)
    out = []
    while nxt:
        start = next(iter(nxt))
        ring, p = [start], start
        while True:
            q = nxt[p].pop()
            if not nxt[p]:
                del nxt[p]
            ring.append(q)
            p = q
            if p == start:
                break
        out.append(ring)
    return out


def ring_area(ring):
    return abs(sum(pa * qd - qa * pd for (pd, pa), (qd, qa) in zip(ring, ring[1:]))) / 2


def main():
    deck = packages(ZONES)
    zs = zones(deck["A-CONFORMOLINES"])
    refs = [r for r, _ in zs]
    assert refs == list(range(1, len(zs) + 1)), "zones are not numbered 1..n"

    with DATA.open() as f:
        rows = list(csv.DictReader(f))
    assert len(rows) == len(zs)
    values = [int(r["dwellings"]) / float(r["area_km2"]) for r in rows]

    rings = outline([ring for _, ring in zs])
    rings = [r for r in rings if ring_area(r) > 0.01]
    centres = [centre(ring) for _, ring in zs]
    total = sum(int(r["dwellings"]) for r in rows)

    out = []
    w = out.append
    for line in HEADER.format(total=f"{total:,}").strip("\n").splitlines():
        w(line)
    w("A-OUTLINE")
    for ring in rings:
        for p in ring:
            w("{:10.2f}{:10.2f}".format(*p).rjust(30))  # rings end at their start
    w("99999")
    w("A-CONFORMOLINES")
    out.extend(deck["A-CONFORMOLINES"])
    w("99999")
    w("B-DATA POINTS")
    for i, (d, a) in enumerate(centres, 1):
        w(f"{i:5d}     {d:10.2f}{a:10.2f}")
    w("99999")
    w("C-OTOLEGENDS")
    out.extend(legends(deck["C-OTOLEGENDS"]))
    w("99999")
    w("E-VALUES")
    for v in values:
        w(f"{v:20.1f}")
    w("99999")
    levels = "LEVELS: 0-1,000-2,000-3,000-4,000-6,000-10,000 DWELLINGS PER"
    for title3, extra in (
        # 25: no data points on the conformant map. 8: no white lines
        # between contour levels, so the tones run continuously.
        ("CONFORMANT MAP OF DUBLIN CITY'S ELECTORAL DIVISIONS", ["   25"]),
        ("CONTOUR MAP FORCED FROM THE SAME ZONES (ELECTIVE 27)", ["    8", "   27"]),
    ):
        w("F-MAP")
        w("CENTRAL DUBLIN - HOUSING DENSITY, CENSUS 2022")
        w("PERMANENT DWELLINGS PER SQUARE KILOMETRE")
        w(title3)
        size = ((DOWN[1] - DOWN[0]) * SCALE, (ACROSS[1] - ACROSS[0]) * SCALE)
        w("    1     " + "".join(f"{v:10.2f}" for v in size))
        w("    2     " + "".join(f"{v:10.2f}" for v in (DOWN[0], ACROSS[0], DOWN[1], ACROSS[1])))
        w("    3             6.")
        w("    4           0.00")
        w(f"    5    {VMAX:11.2f}")
        w("    6" + "".join(f"{v:10.2f}" for v in INTERVALS))
        out.extend(extra)
        w("   10")
        w(levels)
        w("SQUARE KILOMETRE. ~ IS THE RIVER LIFFEY.")
        w("9999")
        w("99999")
    w("999999")
    OUT.write_text("\n".join(out) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}: {len(zs)} zones, {len(rings)} outline rings")


HEADER = """
# Dublin City: housing density (permanent dwellings per square kilometre)
# by Electoral Division, Census 2022, as a conformant map and as a contour
# map forced from the same zones (elective 27), to show how the two differ.
#
# Values: permanent dwellings (T6_8_T) divided by each ED's AREA field,
#   for the 162 CSO Electoral Divisions of Dublin City Council.
#   'Census 2022 - Theme 6 Table 8 - Occupancy status of permanent
#   dwellings on Census night by Electoral Divisions', Central Statistics
#   Office (CSO) Census Hub. https://services-eu1.arcgis.com/
#   BuS9rtTsYEV5C0xh/arcgis/rest/services/CensusHub2022_T6_8_ED/FeatureServer
#   Source: CSO / Tailte Eireann, licensed under Creative Commons
#   Attribution 4.0 (CC BY 4.0). https://creativecommons.org/licenses/by/4.0/
# River and zone outlines: as in dublin-density.sym, whose header lists
#   their sources (CSO / Tailte Eireann and EPA, CC BY 4.0).
# Changes: densities are computed here from the published counts and each
# ED's area; the city outline is the zone edges no two zones share; each
# zone's data point is its area centroid, or the middle of its widest
# span when the centroid falls outside it. {total} dwellings in all.
# Coordinates are Irish Transverse Mercator kilometres: down from
# northing 742 km, across from easting 706.9 km; the map prints the
# window from 3.5 to 10.5 km down and 3.0 to 13.8 km across at 1.2 inch
# per km, twice the scale of dublin-density.sym, so that small EDs keep
# more than their one-character boundary.
# Zones and data points are in ED_ID_STR order; values follow them.
"""

if __name__ == "__main__":
    main()
