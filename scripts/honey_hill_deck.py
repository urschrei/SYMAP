"""Write examples/decks/honey-hill.sym from examples/data/honey-hill-*.

Summer and winter plan zones for a reservoir park at Honey Hill, Swanzey, New
Hampshire, after the Harvard study for the Corps of Engineers (Murray,
Rogers, Sinton, Steinitz, Toth and Way, 1971). As there, each use has an
attractiveness model and the site a vulnerability model, scored per grid
cell; cells are then given to uses. Here a simple greedy allocation does
what the team's designers (and Rogers's linear program) did: each use takes
compact sites, grown cell by cell from the most attractive free cell.

    python3 scripts/honey_hill_deck.py
"""

import csv
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CELLS = ROOT / "examples/data/honey-hill-cells.csv"
LINES = ROOT / "examples/data/honey-hill-lines.json"
OUT = ROOT / "examples/decks/honey-hill.sym"

# The frame of scripts/honey_hill_cells.py: UTM 18N metres, 72 x 72 cells.
LEFT, TOP = 718_400.0, 4_751_900.0
COLS, ROWS = 72, 72
NEIGHBOURS = [(dr, dc) for dr in (-1, 0, 1) for dc in (-1, 0, 1) if dr or dc]
SIDES = [(-1, 0), (1, 0), (0, -1), (0, 1)]

# Zones: code, name, symbol strikes (bottom first). The codes are the
# values mapped, one level each, in the order of the map's levels.
SUMMER = [
    (1, "SUMMER POOL", "-"),
    (2, "FLOOD FRINGE", "."),
    (3, "OPEN SPACE", ""),
    (4, "NATURE PRESERVE", "'"),
    (5, "EXISTING BUILDINGS", "/"),
    (6, "CAMPING", "O"),
    (7, "PICNIC AND DAY USE", "X"),
    (8, "BEACH AND SWIMMING", "OXAV"),
]
WINTER = [
    (1, "ICE", "-"),
    (2, "FLOOD FRINGE", "."),
    (3, "OPEN SPACE", ""),
    (4, "NATURE PRESERVE", "'"),
    (5, "EXISTING BUILDINGS", "/"),
    (6, "SKI TOURING", "+"),
    (7, "SKATING AND ICE FISHING", "O"),
    (8, "WINTER DAY USE", "X"),
    (9, "SKI SLOPES", "OXAV"),
]


def load():
    grid = {}
    for r in csv.DictReader(CELLS.open()):
        k = (int(r["row"]), int(r["col"]))
        grid[k] = {n: float(v) for n, v in r.items() if n not in ("row", "col")}
    return grid


def facing(aspect, towards):
    """1 facing `towards` (degrees), -1 facing away."""
    return math.cos(math.radians(aspect - towards))


def vulnerability(c):
    """0 to 100: the harm intensive use would do."""
    v = 120 * c["wetland"]
    v = max(v, 4 * (c["slope"] - 25))
    v = max(v, 40 * c["stream"], 60 * c["conservation"])
    return min(v, 100)


def clumps(cells, least):
    """The cells in 8-connected groups of at least `least` cells."""
    out, seen = set(), set()
    for k in cells:
        if k in seen:
            continue
        group, todo = {k}, [k]
        while todo:
            r, c = todo.pop()
            for dr, dc in NEIGHBOURS:
                n = (r + dr, c + dc)
                if n in cells and n not in group:
                    group.add(n)
                    todo.append(n)
        seen |= group
        if len(group) >= least:
            out |= group
    return out


def allocate(grid, free, score, sites, size):
    """Grow `sites` compact sites of up to `size` cells, best cells first.

    `score` gives a free cell's attractiveness, or None where the use cannot
    go. Each site starts at the best remaining cell and adds the best cell
    on its edge (favouring cells that share sides with the site) until it
    is full or no edge cell can take the use.
    """
    scored = {k: s for k in free if (s := score(grid[k])) is not None}
    taken = set()
    for _ in range(sites):
        open_ = [k for k in scored if k not in taken]
        if not open_:
            break
        site = {max(open_, key=lambda k: (scored[k], k))}
        while len(site) < size:
            edge = {
                (r + dr, c + dc)
                for r, c in site
                for dr, dc in NEIGHBOURS
                if (r + dr, c + dc) in scored
                and (r + dr, c + dc) not in site
                and (r + dr, c + dc) not in taken
            }
            if not edge:
                break
            # Prefer cells that close the site up: 15 points a side shared.
            def fit(k):
                shared = sum((k[0] + dr, k[1] + dc) in site for dr, dc in SIDES)
                return (scored[k] + 15 * shared, k)

            site.add(max(edge, key=fit))
        taken |= site
    return taken


def plans(grid, dam_row):
    """Each cell's zone code in the summer and the winter plan."""
    water = {k for k, c in grid.items() if c["summer"] >= 0.5}
    land = set(grid) - water
    built = {k for k in land if grid[k]["developed"] >= 0.4}
    preserve = clumps({k for k in land - built if vulnerability(grid[k]) >= 50}, 3)
    fringe = {k for k in land - built - preserve if grid[k]["flood"] >= 0.5}
    # The park is the land south of the dam.
    free = {k for k in land - built - preserve if k[0] > dam_row}

    def beach(c):
        if c["d_shore"] > 120 or c["slope"] > 8 or c["wetland"] > 0.2:
            return None
        return 100 - 6 * c["slope"] - c["d_road"] / 20 + 15 * facing(c["aspect"], 200) + 10 * c["open"]

    def picnic(c):
        if c["d_shore"] > 500 or c["slope"] > 10 or c["flood"] > 0.5 or c["d_road"] > 800:
            return None
        return 100 - 5 * c["slope"] - c["d_shore"] / 10 - c["d_road"] / 15 + 20 * c["open"] + 10 * c["forest"]

    def camping(c):
        if not (150 <= c["d_shore"] <= 1500) or c["slope"] > 15 or c["forest"] < 0.3:
            return None
        if c["flood"] > 0.1 or not (100 <= c["d_road"] <= 1500):
            return None
        return 100 - 4 * c["slope"] - abs(c["d_shore"] - 500) / 20 + 20 * c["forest"] - c["d_road"] / 40

    summer = {}
    beaches = allocate(grid, free, beach, 4, 6)
    days = allocate(grid, free - beaches, picnic, 4, 15)
    camps = allocate(grid, free - beaches - days, camping, 4, 35)
    for k in grid:
        summer[k] = (
            1 if k in water else 5 if k in built else 4 if k in preserve
            else 8 if k in beaches else 7 if k in days else 6 if k in camps
            else 2 if k in fringe else 3
        )

    def ski(c):
        if not (15 <= c["slope"] <= 45) or c["relief"] < 50 or c["conservation"] > 0.5:
            return None
        if facing(c["aspect"], 20) < 0.2:
            return None
        return c["relief"] / 2 + 30 * facing(c["aspect"], 20) - abs(c["slope"] - 28) - c["d_road"] / 50

    def touring(c):
        if c["slope"] > 15 or c["d_shore"] > 1500 or c["flood"] > 0.5:
            return None
        return 100 - 3 * c["slope"] + 20 * c["forest"] - c["d_shore"] / 30

    slopes = allocate(grid, free, ski, 1, 45)
    # The winter day use areas are the summer ones nearest a road, with the
    # cells next to the ski slopes.
    lodge = allocate(grid, days, lambda c: 100 - c["d_road"] / 10, 2, 10)
    base = {
        (r + dr, c + dc)
        for r, c in slopes
        for dr, dc in NEIGHBOURS
        if (r + dr, c + dc) in free - slopes and grid[(r + dr, c + dc)]["slope"] < 15
    }
    lodge |= set(sorted(base, key=lambda k: grid[k]["d_road"])[:6])
    trails = allocate(grid, free - slopes - lodge, touring, 3, 70)
    rink = {
        (r, c)
        for r, c in water
        if any(abs(r - r2) <= 2 and abs(c - c2) <= 3 for r2, c2 in lodge)
    }
    winter = {}
    for k in grid:
        winter[k] = (
            7 if k in rink else 1 if k in water else 5 if k in built
            else 4 if k in preserve else 9 if k in slopes else 8 if k in lodge
            else 6 if k in trails else 2 if k in fringe else 3
        )
    return summer, winter


def km(x, y):
    """Down and across, in km from the top left corner."""
    return (TOP - y) / 1000, (x - LEFT) / 1000


def line_legends(symbol, geom):
    out = []
    parts = geom["coordinates"] if geom["type"] == "MultiLineString" else [geom["coordinates"]]
    for part in parts:
        for j, (x, y) in enumerate(part):
            d, a = km(x, y)
            lead = f"     {symbol:<4}L" if j == 0 else ""
            out.append(f"{lead:<10}{d:10.2f}{a:10.2f}")
    return out


def symbolism(zones):
    """Elective 7's four cards: each level's strikes, for areas and points."""
    cards = []
    for s in range(4):
        level = "".join((z[2][s] if s < len(z[2]) else " ") for z in zones)
        cards.append(f"{level:<10}{level:<10}".rstrip() or " ")
    return cards


# North arrow, scale bar and a label for Honey Hill (its summit is the cell
# at row 41, column 29), and a cleared inset in the empty south-east corner.
FURNITURE = """
   10    P      5.19      1.75
HONEY HILL
         A      7.70      5.60
                7.70      7.15
                8.95      7.15
                8.95      5.60
                7.70      5.60
     *   P      7.90      5.70       0.0       0.0
     *   P      7.90      5.70       1.0      -1.0
     *   P      7.90      5.70       1.0       0.0
     *   P      7.90      5.70       1.0       1.0
     I   P      7.90      5.70       2.0
     I   P      7.90      5.70       3.0
     N   P      7.90      5.70       5.0
   11    P      8.45      5.95      -3.0
SCALE IN KM
     =   L      8.45      5.95
                8.45      6.95
     I   P      8.45      5.95
    1    P      8.45      5.95      -1.0
0
     I   P      8.45      6.95
    1    P      8.45      6.95      -1.0
1
"""

DECK_HEADER = """
# Honey Hill: summer and winter plan zones for a reservoir park in Swanzey,
# New Hampshire, after the Harvard study for the US Army Corps of Engineers.
# Generated by scripts/honey_hill_deck.py from examples/data/honey-hill-*
# (scripts/honey_hill_cells.py).
#
# The study: T. Murray, P. Rogers, D. Sinton, C. Steinitz, R. Toth and D.
#   Way, 'Honey Hill: A Systems Analysis for Planning the Multiple Use of
#   Controlled Water Areas', IWR Report 71-9 (2 vols., US Army Corps of
#   Engineers Institute for Water Resources, 1971). Harvard's Landscape
#   Architecture Research Office scored a proposed flood control reservoir
#   and new state park, cell by cell, for the attractiveness of the land to
#   each recreation use and its vulnerability to harm; each team member
#   designed the lake and park for summer and winter, and the plans were
#   printed as grid maps (C. Steinitz, 'Beginnings of geodesign: a personal
#   historical perspective', Research in Urbanism Series 4 (2016), 9-24,
#   doi:10.7480/rius.4.1366, figure 7). The report was not available for
#   this deck, so the uses and rules below are this deck's, not the study's.
# The site: a Keene Sentinel 'This day in history' item (21 November),
#   seen only in a search summary, says the Honey Hill flood control dam,
#   across Route 12 and the South Branch of the Ashuelot in Swanzey, was
#   approved by Congress in 1941 and never built. Its axis and pools are not
#   known here. This deck models an earth dam across the South Branch just
#   south of Swanzey Center (the heavy line at the top), where the valley
#   narrows north of Honey Hill, with a summer pool at 500 ft and a flood
#   pool at 520 ft (the basin spills at about 530 ft). Route 12 follows the
#   South Branch further upstream, so the real dam may have stood there.
# Data: USGS 3DEP 1/3 arc-second elevation (public domain); NLCD 2021 land
#   cover, MRLC (public domain); roads and rivers from OpenStreetMap, (c)
#   OpenStreetMap contributors, ODbL; conservation lands from NH GRANIT.
#   Fetched 23 November 2026.
# Model, per cell: VULNERABILITY is the worst of wetland share, slope over
#   25%, a mapped stream and conservation land; groups of three or more
#   cells scoring 50 or more are NATURE PRESERVE. Built-up cells (NLCD
#   22-24 over 40%) stay as they are. Uses go only south of the dam.
#   BEACH: within 120 m of the summer shore, slope under 8%, facing
#   south-west, near a road. PICNIC: within 500 m of the shore, above the
#   flood pool, slope under 10%, open land, near a road. CAMPING: wooded
#   land 150 m to 1.5 km from the shore, above the flood pool, slope under
#   15%. SKI SLOPES: slopes of 15-45% facing north to east, 50 m or more
#   above the lowest point within 500 m. SKI TOURING: gentle land within
#   1.5 km of the shore, forest preferred. WINTER DAY USE: the picnic areas
#   nearest a road and the foot of the ski slopes; SKATING is the ice beside
#   them. Each use takes a few compact sites grown from its best cells.
# Grid: one data point at the centre of every printed character, 100 m
#   across and 125 m down (UTM zone 18N), as the Laboratory's GRID program
#   printed cells. The proximal map then gives each character its own cell;
#   each level's data point symbol is its zone symbol. 1 inch to 1 km.
"""


def main():
    grid = load()
    lines = {f["properties"]["layer"]: f["geometry"] for f in json.load(LINES.open())["features"]}
    dam = lines["dam"]["coordinates"][0]
    dam_row = int(max(km(x, y)[0] for x, y in dam) * 8)
    summer, winter = plans(grid, dam_row)
    keys = sorted(grid)
    out = []
    w = out.append
    w(DECK_HEADER.strip())
    w("B-DATA POINTS")
    for r, c in keys:
        w(f"{'':10}{(r + 0.5) / 8:10.4f}{(c + 0.5) / 10:10.4f}")
    w("99999")
    w("C-OTOLEGENDS")
    for layer, sym in [("streams", "~"), ("roads", "="), ("dam", "OXAV")]:
        out.extend(line_legends(sym, lines[layer]))
    out.extend(FURNITURE.strip("\n").splitlines())
    w("99999")
    for zones, plan, season in [(SUMMER, summer, "SUMMER"), (WINTER, winter, "WINTER")]:
        w("E-VALUES")
        for k in keys:
            w(f"{plan[k]:20.1f}")
        w("99999")
        n = len(zones)
        w("F-MAP")
        w(f"HONEY HILL, SWANZEY, NEW HAMPSHIRE - {season} PLAN")
        w("RESERVOIR PARK ZONES ON A 100 M GRID, AFTER HARVARD AND THE CORPS, 1971")
        w("PROXIMAL MAP OF 5,184 CELLS")
        w("    1           9.00      7.20")
        w("    2           0.00      0.00      9.00      7.20")
        w(f"    3             {n}.")
        w("    4           0.50")
        w(f"    5           {n + 0.5:.2f}")
        w("    7")
        out.extend(symbolism(zones))
        w("    8")
        w("    9")
        w("   17")
        w("   10")
        legend = [f"{code} {name}" for code, name, _ in zones]
        text, cur = [], ""
        for item in legend:
            nxt = f"{cur}   {item}" if cur else item
            if len(nxt) > 72:
                text.append(cur)
                nxt = item
            cur = nxt
        text.append(cur)
        out.extend(text)
        w("SUMMER POOL 500 FT, FLOOD POOL 520 FT. = MAIN ROAD   ~ RIVER")
        w("HEAVY LINE AT TOP: DAM. 1 INCH TO 1 KM.")
        w("THE ZONES ARE A PRESENT-DAY MODEL, NOT THE 1971 PLANS.")
        w("9999")
        w("   31")
        w("   36")
        w("   37")
        w("99999")
        counts = {z[1]: sum(v == z[0] for v in plan.values()) for z in zones}
        print(season, counts, file=sys.stderr)
    w("999999")
    OUT.write_text("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
