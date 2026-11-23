#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy", "pyproj", "rasterio", "scipy", "shapely"]
# ///
"""Build the character-cell grid of site factors for the Honey Hill deck.

The grid covers 7.2 km by 9 km of Swanzey, New Hampshire, around the site of
the Corps of Engineers' Honey Hill reservoir (authorised 1941, never built).
Each cell is one printed character at 1 inch to 1 km (10 characters and 8
lines to the inch): 100 m across and 125 m down, in UTM zone 18N
(EPSG:32618). Factors are worked out on a 10 m by 10.42 m grid, 10 by 12 of
its pixels to a cell.

The reservoir is modelled, not surveyed: an earth dam across the South
Branch of the Ashuelot just south of Swanzey Center (DAM below), a summer
pool at 500 ft (152.4 m) and a flood pool at 520 ft (158.5 m), filled over
the 3DEP elevation model from a point on the river upstream. The basin's
lowest saddle is about 161.7 m, so neither pool spills.

    uv run scripts/honey_hill_cells.py

Output (examples/data/):
  honey-hill-cells.csv   row, col (from the top left), elevation (m), slope
      (%), aspect (degrees clockwise from north, of the downhill direction),
      relief (m above the lowest point within 500 m), the cell's share in the
      summer pool and in the flood pool, its share of NLCD 2021 forest (41-43),
      open land (31, 52, 71, 81, 82), wetland (90, 95) and developed land
      (22-24), the distance to the nearest public road outside the flood pool
      and to the summer shoreline (m), its share of NH conservation land, and
      whether a mapped stream crosses it.
  honey-hill-lines.json  the summer shoreline, the flood pool limit, the dam,
      main roads and streams outside the summer pool, in UTM 18N metres.
"""

import argparse
import csv
import json
from pathlib import Path

import numpy as np
import rasterio
import shapely
from pyproj import Transformer
from rasterio.features import rasterize, shapes
from rasterio.transform import Affine
from rasterio.warp import Resampling, reproject
from scipy import ndimage
from shapely.geometry import LineString, MultiLineString, mapping, shape
from shapely.ops import unary_union

# Map frame, UTM 18N metres: 72 columns by 72 lines of characters.
LEFT, TOP = 718_400.0, 4_751_900.0
COLS, ROWS = 72, 72
CELL_W, CELL_H = 100.0, 125.0
PX_W, PX_H = 10.0, CELL_H / 12

SUMMER_POOL, FLOOD_POOL = 152.4, 158.5  # 500 and 520 ft
# Dam axis (lon, lat) and a seed on the river upstream of it.
DAM = [(-72.2905, 42.8768), (-72.2700, 42.8774)]
SEED = (-72.2743, 42.8549)

FOREST = {41, 42, 43}
OPEN = {31, 52, 71, 81, 82}
WETLAND = {90, 95}
DEVELOPED = {22, 23, 24}
ROADS = {
    "trunk", "primary", "secondary", "tertiary", "unclassified", "residential",
    "trunk_link", "primary_link", "secondary_link", "tertiary_link",
}
MAIN_ROADS = {"trunk", "primary", "secondary"}

to_utm = Transformer.from_crs("EPSG:4326", "EPSG:32618", always_xy=True).transform


def parse_args():
    data = Path("temp/honeyhill")
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--dem", type=Path, default=data / "dem.tif")
    p.add_argument("--landcover", type=Path, default=data / "landcover.tif")
    p.add_argument("--osm", type=Path, default=data / "osm.json")
    p.add_argument(
        "--conservation", type=Path, default=data / "granit_conservation.geojson"
    )
    p.add_argument("--out", type=Path, default=Path("examples/data"))
    return p.parse_args()


def warp(path, resampling, margin=0):
    """Read a raster onto the working grid, `margin` pixels wider each side."""
    h, w = ROWS * 12 + 2 * margin, COLS * 10 + 2 * margin
    t = Affine(PX_W, 0, LEFT - margin * PX_W, 0, -PX_H, TOP + margin * PX_H)
    out = np.full((h, w), np.nan, dtype="float32")
    with rasterio.open(path) as src:
        reproject(
            rasterio.band(src, 1),
            out,
            dst_transform=t,
            dst_crs="EPSG:32618",
            resampling=resampling,
            dst_nodata=np.nan,
        )
    return out, t


def pixel(t, x, y):
    c, r = ~t * (x, y)
    return int(r), int(c)


def pool(z, t, level):
    """The cells below `level` connected to the seed, the dam blocking."""
    dam = LineString([to_utm(*p) for p in DAM]).buffer(PX_W)
    blocked = rasterize([dam], out_shape=z.shape, transform=t).astype(bool)
    lab, _ = ndimage.label((z < level) & ~blocked)
    k = lab[pixel(t, *to_utm(*SEED))]
    assert k, "seed is above the pool"
    m = lab == k
    assert not (m[0].any() or m[-1].any() or m[:, 0].any() or m[:, -1].any()), (
        "pool reaches the frame edge"
    )
    return m


def cells(a, f=np.nanmean):
    """Aggregate a working-grid array to the character cells."""
    return f(a.reshape(ROWS, 12, COLS, 10), axis=(1, 3))


def edges(mask, t):
    """The boundary of a mask as lines, in UTM metres, simplified to 15 m."""
    polys = [
        shape(g) for g, v in shapes(mask.astype("uint8"), transform=t) if v == 1
    ]
    rings = []
    for p in polys:
        if p.area < 20_000:
            continue
        rings.append(p.exterior)
        rings.extend(i for i in p.interiors if i.length > 400)
    return MultiLineString([r.simplify(15) for r in rings])


def main():
    args = parse_args()
    m = 50  # 500 m margin for relief
    zm, tm = warp(args.dem, Resampling.bilinear, margin=m)
    z = zm[m:-m, m:-m]
    t = tm * Affine.translation(m, m)
    gy, gx = np.gradient(z, PX_H, PX_W)  # rows run south
    slope = 100 * np.hypot(gx, gy)
    relief = zm - ndimage.minimum_filter(np.nan_to_num(zm, nan=1e4), size=(97, 101))
    relief = relief[m:-m, m:-m]

    summer, flood = pool(z, t, SUMMER_POOL), pool(z, t, FLOOD_POOL)
    lc, _ = warp(args.landcover, Resampling.nearest)

    osm = json.load(open(args.osm))["elements"]

    def lines(test):
        out = []
        for e in osm:
            tags, g = e.get("tags", {}), e.get("geometry")
            if g and e["type"] == "way" and test(tags):
                out.append(LineString([to_utm(p["lon"], p["lat"]) for p in g]))
        return out

    roads = lines(lambda tg: tg.get("highway") in ROADS)
    main = lines(lambda tg: tg.get("highway") in MAIN_ROADS)
    streams = lines(lambda tg: tg.get("waterway") in ("river", "stream"))

    road_px = rasterize(roads, out_shape=z.shape, transform=t, all_touched=True)
    road_px = road_px.astype(bool) & ~flood
    d_road = ndimage.distance_transform_edt(~road_px, sampling=(PX_H, PX_W))
    d_shore = ndimage.distance_transform_edt(~summer, sampling=(PX_H, PX_W))
    stream_px = rasterize(streams, out_shape=z.shape, transform=t, all_touched=True)

    cons = json.load(open(args.conservation))["features"]
    cons_geoms = [
        shapely.transform(shape(f["geometry"]), lambda xy: np.column_stack(to_utm(*xy.T)))
        for f in cons
    ]
    cons_px = rasterize(
        [g for g in cons_geoms if not g.is_empty], out_shape=z.shape, transform=t
    )

    share = lambda codes: cells(np.isin(lc, list(codes)).astype(float))
    # Aspect of the cell's mean gradient: the downhill direction.
    gxc, gyc = cells(gx), cells(gy)
    aspect = (np.degrees(np.arctan2(-gxc, gyc)) + 360) % 360
    cols = {
        "elev": cells(z),
        "slope": cells(slope),
        "aspect": aspect,
        "relief": cells(relief),
        "summer": cells(summer.astype(float)),
        "flood": cells(flood.astype(float)),
        "forest": share(FOREST),
        "open": share(OPEN),
        "wetland": share(WETLAND),
        "developed": share(DEVELOPED),
        "d_road": cells(d_road),
        "d_shore": cells(d_shore),
        "conservation": cells(cons_px.astype(float)),
        "stream": cells(stream_px.astype(float), np.max),
    }
    fmt = {"elev": 1, "slope": 1, "aspect": 0, "relief": 1, "d_road": 0, "d_shore": 0}
    args.out.mkdir(parents=True, exist_ok=True)
    with open(args.out / "honey-hill-cells.csv", "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["row", "col", *cols])
        for r in range(ROWS):
            for c in range(COLS):
                vals = []
                for k, a in cols.items():
                    v = float(a[r, c])
                    vals.append(f"{v:.{fmt.get(k, 2)}f}")
                w.writerow([r, c, *vals])

    frame = shape(
        {
            "type": "Polygon",
            "coordinates": [[
                (LEFT, TOP), (LEFT + COLS * CELL_W, TOP),
                (LEFT + COLS * CELL_W, TOP - ROWS * CELL_H),
                (LEFT, TOP - ROWS * CELL_H), (LEFT, TOP),
            ]],
        }
    )
    lake = unary_union(
        [shape(g) for g, v in shapes(summer.astype("uint8"), transform=t) if v == 1]
    ).buffer(0)
    clip = lambda ls: [g for g in (l.intersection(frame).difference(lake) for l in ls) if not g.is_empty]
    layers = {
        "shore": edges(summer, t),
        "flood": edges(flood, t),
        "dam": MultiLineString([LineString([to_utm(*p) for p in DAM])]),
        "roads": unary_union(clip(main)).simplify(20),
        "streams": unary_union(clip(lines(lambda tg: tg.get("waterway") == "river"))).simplify(20),
    }
    out = {
        "type": "FeatureCollection",
        "crs": "EPSG:32618",
        "features": [
            {"type": "Feature", "properties": {"layer": k}, "geometry": mapping(g)}
            for k, g in layers.items()
        ],
    }
    rnd = lambda o: [rnd(x) for x in o] if isinstance(o, (list, tuple)) else round(o)
    for f in out["features"]:
        f["geometry"]["coordinates"] = rnd(f["geometry"]["coordinates"])
    (args.out / "honey-hill-lines.json").write_text(json.dumps(out) + "\n")
    print(
        f"summer pool {summer.sum() * PX_W * PX_H / 1e6:.1f} km2, "
        f"flood pool {flood.sum() * PX_W * PX_H / 1e6:.1f} km2"
    )


if __name__ == "__main__":
    main()
