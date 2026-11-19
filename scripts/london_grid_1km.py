#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["geopandas", "pyogrio", "shapely"]
# ///
"""Build a 1 km British National Grid of site factors for Greater London.

Each output row is a 1 km cell (corners on multiples of 1000 m) of which at
least half lies inside the Greater London boundary:

    x, y      cell centre, metres (EPSG:27700)
    inside    share of the cell inside London
    ptal_ai   mean PTAL 2023 access index of the 100 m cells whose centres
              fall in the cell
    n_ptal    number of those 100 m cells
    flood     share of the cell in Flood Zone 2 or 3
    no2       LAEI 2019 annual mean NO2 for the cell (matched on SW corner)
    green     share of the cell that is public greenspace

    uv run scripts/london_grid_1km.py --out examples/data/london-grid-1km.csv
"""

import argparse
import csv
import math
import statistics
from pathlib import Path

import geopandas as gpd
import pandas as pd
import shapely
from shapely.geometry import box

CELL = 1000
GREEN_FUNCTIONS = {
    "Public Park Or Garden",
    "Playing Field",
    "Play Space",
    "Other Sports Facility",
}


def parse_args():
    data = Path("temp/delmarva")
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument(
        "--boundary", type=Path, default=data / "greater_london_boundary.geojson"
    )
    p.add_argument("--ptal", type=Path, default=data / "ptal2023_100m.csv")
    p.add_argument(
        "--flood", type=Path, default=data / "flood_zones_2_3_london.geojson"
    )
    p.add_argument("--no2", type=Path, default=data / "no2_laei2019_1km.csv")
    p.add_argument(
        "--greenspace", type=Path, default=data / "os_open_greenspace_london.geojson"
    )
    p.add_argument(
        "--out", type=Path, default=Path("examples/data/london-grid-1km.csv")
    )
    p.add_argument("--min-inside", type=float, default=0.5)
    return p.parse_args()


def dissolve(gdf):
    """Union all geometries into one valid geometry."""
    geoms = shapely.make_valid(gdf.geometry.values)
    return shapely.union_all(geoms)


def shares(cells, geom):
    """Return the share of each cell's area that lies inside geom."""
    shapely.prepare(geom)
    out = []
    for cell in cells:
        if not geom.intersects(cell):
            out.append(0.0)
        elif geom.contains(cell):
            out.append(1.0)
        else:
            out.append(geom.intersection(cell).area / cell.area)
    return out


def main():
    args = parse_args()

    london = dissolve(gpd.read_file(args.boundary))
    minx, miny, maxx, maxy = london.bounds
    x0 = math.floor(minx / CELL) * CELL
    y0 = math.floor(miny / CELL) * CELL
    x1 = math.ceil(maxx / CELL) * CELL
    y1 = math.ceil(maxy / CELL) * CELL
    corners = [(x, y) for y in range(y0, y1, CELL) for x in range(x0, x1, CELL)]
    cells = [box(x, y, x + CELL, y + CELL) for x, y in corners]

    inside = shares(cells, london)
    keep = [i for i, s in enumerate(inside) if s >= args.min_inside]
    corners = [corners[i] for i in keep]
    cells = [cells[i] for i in keep]
    inside = [inside[i] for i in keep]

    # PTAL: bin each 100 m cell by the 1 km cell that holds its centre.
    ptal = pd.read_csv(args.ptal, usecols=["x", "y", "ai"])
    ptal["cx"] = (ptal["x"] // CELL * CELL).astype(int)
    ptal["cy"] = (ptal["y"] // CELL * CELL).astype(int)
    ptal_by_cell = ptal.groupby(["cx", "cy"])["ai"].agg(["mean", "count"])

    no2 = pd.read_csv(args.no2, usecols=["x", "y", "no2"])
    no2_by_cell = {(int(r.x), int(r.y)): r.no2 for r in no2.itertuples()}

    flood_geom = dissolve(gpd.read_file(args.flood))
    flood = shares(cells, flood_geom)

    green_gdf = gpd.read_file(args.greenspace)
    green_gdf = green_gdf[green_gdf["function"].isin(GREEN_FUNCTIONS)]
    green_geom = dissolve(green_gdf)
    green = shares(cells, green_geom)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["x", "y", "inside", "ptal_ai", "n_ptal", "flood", "no2", "green"])
        for i, (x, y) in enumerate(corners):
            key = (x, y)
            if key in ptal_by_cell.index:
                ai = f"{ptal_by_cell.loc[key, 'mean']:.2f}"
                n = int(ptal_by_cell.loc[key, "count"])
            else:
                ai, n = "", 0
            v = no2_by_cell.get(key)
            w.writerow(
                [
                    x + CELL // 2,
                    y + CELL // 2,
                    f"{inside[i]:.3f}",
                    ai,
                    n,
                    f"{flood[i]:.3f}",
                    "" if v is None else f"{v:.1f}",
                    f"{green[i]:.3f}",
                ]
            )

    print(f"wrote {len(corners)} rows to {args.out}")
    df = pd.read_csv(args.out)
    for col in df.columns:
        s = df[col].dropna()
        print(
            f"{col:8} min {s.min():>10} median {statistics.median(s):>10} max {s.max():>10} missing {df[col].isna().sum()}"
        )
    missing = df[df["ptal_ai"].isna() | df["no2"].isna()]
    if len(missing):
        print("cells with missing ptal_ai or no2:")
        print(missing.to_string(index=False))


if __name__ == "__main__":
    main()
