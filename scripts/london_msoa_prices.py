#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["pandas", "python-calamine"]
# ///
"""Build a table of median house prices for the MSOAs of Greater London.

Each output row is one 2021 MSOA in a London borough (LAD code E09*):

    msoa21cd  MSOA (December 2021) code
    name      MSOA (December 2021) name
    x, y      MSOA centroid, metres (EPSG:27700), from the BNG_E and BNG_N
              fields of the 2021 BSC boundaries
    price     median price paid, pounds, all dwellings, for the latest
              year-ending period in HPSSA dataset 2 (table 1a)
    sales     number of sales, all dwellings, for the same period, from
              HPSSA dataset 1 (table 1a)

HPSSA uses 2011 MSOAs. Only the 2011 MSOAs that the exact-fit lookup marks
as unchanged ("U") in 2021 are kept, because a split or merged area has no
single price for its 2021 successor. Rows with a suppressed price (":",
fewer than five sales) are dropped.

    uv run scripts/london_msoa_prices.py --out examples/data/london-msoa-prices.csv
"""

import argparse
import csv
import statistics
from pathlib import Path

import pandas as pd

HPSSA2 = "hpssa2/HPSSA Dataset 2 - Median price paid by MSOA.xls"
HPSSA1 = "hpssa1/HPSSA Dataset 1 - Number of residential property sales by MSOA.xls"
LOOKUP = "MSOA11_MSOA21_LAD22_EW_LU_exact.csv"
CENTROIDS = "MSOA_2021_EW_BSC_V3.csv"


def latest(path: Path) -> tuple[str, pd.Series]:
    """Return the label of the latest period in table 1a and its values by MSOA11 code."""
    table = pd.read_excel(path, sheet_name="1a", header=5, engine="calamine", dtype=str)
    table = table[table["Local authority code"].str.startswith("E09", na=False)]
    column = [c for c in table.columns if str(c).startswith("Year ending")][-1]
    values = pd.to_numeric(table[column].str.replace(",", ""), errors="coerce")
    return column, pd.Series(values.values, index=table["MSOA code"].values)


def main() -> None:
    data = Path("temp/regression")
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument(
        "--data", type=Path, default=data, help="directory of downloaded inputs"
    )
    p.add_argument(
        "--out", type=Path, default=Path("examples/data/london-msoa-prices.csv")
    )
    args = p.parse_args()

    period, price = latest(args.data / HPSSA2)
    period_sales, sales = latest(args.data / HPSSA1)
    if period != period_sales:
        raise SystemExit(f"period mismatch: {period!r} and {period_sales!r}")

    lookup = pd.read_csv(args.data / LOOKUP, dtype=str)
    lookup = lookup[lookup["LAD22CD"].str.startswith("E09")]
    unchanged = lookup[lookup["CHNGIND"] == "U"].set_index("MSOA11CD")["MSOA21CD"]
    centroids = pd.read_csv(args.data / CENTROIDS, dtype={"MSOA21CD": str}).set_index(
        "MSOA21CD"
    )

    rows = []
    dropped = {"changed": 0, "suppressed": 0}
    for code in price.index:
        if code not in unchanged.index:
            dropped["changed"] += 1
            continue
        if pd.isna(price[code]):
            dropped["suppressed"] += 1
            continue
        msoa21 = unchanged[code]
        c = centroids.loc[msoa21]
        n = sales.get(code)
        rows.append(
            {
                "msoa21cd": msoa21,
                "name": c["MSOA21NM"],
                "x": int(c["BNG_E"]),
                "y": int(c["BNG_N"]),
                "price": int(price[code]),
                "sales": "" if pd.isna(n) else int(n),
            }
        )

    rows.sort(key=lambda r: r["msoa21cd"])
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("w", newline="") as fh:
        w = csv.DictWriter(
            fh, fieldnames=["msoa21cd", "name", "x", "y", "price", "sales"]
        )
        w.writeheader()
        w.writerows(rows)

    prices = [int(r["price"]) for r in rows]
    print(f"{period}: {len(rows)} rows written to {args.out}")
    print(
        f"dropped: {dropped['changed']} split or merged, {dropped['suppressed']} suppressed"
    )
    print(
        f"price min {min(prices)}, median {statistics.median(prices):.0f}, max {max(prices)}"
    )


if __name__ == "__main__":
    main()
