#!/usr/bin/env python3
"""Compare a map in a scan of the 1971 manual with this implementation's output.

Renders the page at 400 dpi, finds the map's border, classifies every
character cell by template matching, and compares it with the map that
`symap` prints for the deck.

    scripts/compare_scan.py manual.pdf 32 examples/decks/example-1-3.sym \
        --bbox 1395,593,4795,2726

--bbox gives the border's approximate left, top, right and bottom in pixels of
the upright 400 dpi page; each edge is refined to within --search pixels.
Needs numpy, Pillow and pdftoppm.
"""

import argparse
import json
import os
import subprocess
import sys
from collections import Counter

import numpy as np
from PIL import Image, ImageFilter

DPI = 400


def render(pdf, page, rotate, cache):
    os.makedirs(cache, exist_ok=True)
    stem = os.path.join(cache, f"p{page}")
    png = f"{stem}-r{rotate}.png"
    if not os.path.exists(png):
        subprocess.run(
            ["pdftoppm", "-f", str(page), "-l", str(page), "-r", str(DPI),
             "-gray", "-singlefile", pdf, stem],
            check=True,
        )
        Image.open(stem + ".pgm").rotate(rotate, expand=True).save(png)
        os.remove(stem + ".pgm")
    return Image.open(png)


def normalise(im):
    """Darkness in [0, 1], with the paper's brightness divided out."""
    bg = im.reduce(16).filter(ImageFilter.MaxFilter(9)).resize(im.size, Image.BILINEAR)
    a = np.asarray(im, dtype=np.float32)
    b = np.asarray(bg, dtype=np.float32) + 1
    return np.clip(1 - a / b, 0, 1)


def fit_edge(dark, approx, lo, hi, search, vertical, pieces=6):
    """Fit the border line near `approx`: x = a + b*y if vertical, else y = a + b*x."""
    xs, ys = [], []
    for k in range(pieces):
        s0 = int(lo + (hi - lo) * k / pieces)
        s1 = int(lo + (hi - lo) * (k + 1) / pieces)
        a0, a1 = approx - search, approx + search
        if vertical:
            prof = dark[s0:s1, a0:a1].sum(0)
        else:
            prof = dark[a0:a1, s0:s1].sum(1)
        prof = np.convolve(prof, np.ones(7) / 7, "same")
        xs.append((s0 + s1) / 2)
        ys.append(a0 + int(prof.argmax()))
    b, a = np.polyfit(xs, ys, 1)
    return a, b


def corners(dark, bbox, search):
    l, t, r, b = bbox
    m = (r - l) // 12, (b - t) // 12
    left = fit_edge(dark, l, t + m[1], b - m[1], search, True)
    right = fit_edge(dark, r, t + m[1], b - m[1], search, True)
    top = fit_edge(dark, t, l + m[0], r - m[0], search, False)
    bottom = fit_edge(dark, b, l + m[0], r - m[0], search, False)

    def meet(v, h):  # x = av + bv*y, y = ah + bh*x
        x = (v[0] + v[1] * h[0]) / (1 - v[1] * h[1])
        return np.array([x, h[0] + h[1] * x])

    return meet(left, top), meet(right, top), meet(left, bottom), meet(right, bottom)


def symap_map(binary, deck, index):
    out = subprocess.run([binary, deck], check=True, capture_output=True, text=True).stdout
    lines = out.replace("\f", "").split("\n")
    starts = [i for i, s in enumerate(lines) if s.startswith("*") and s.rstrip().endswith("*")]
    tops = starts[0::2]
    t = tops[index]
    b = starts[starts.index(t) + 1]
    rows = lines[t:b + 1]
    w = len(rows[0])
    return [list(s.ljust(w)[:w]) for s in rows]


def cells(dark, tl, tr, bl, br, rows, cols, size=(14, 18)):
    """Each cell's patch, resampled to size, with the cell pitch."""
    pitch_x = np.linalg.norm(tr - tl) / (cols - 1)
    pitch_y = np.linalg.norm(bl - tl) / (rows - 1)
    img = Image.fromarray((dark * 255).astype(np.uint8))
    out = np.zeros((rows, cols, size[1], size[0]), dtype=np.float32)
    for i in range(rows):
        v = i / (rows - 1)
        for j in range(cols):
            u = j / (cols - 1)
            c = (1 - u) * (1 - v) * tl + u * (1 - v) * tr + (1 - u) * v * bl + u * v * br
            box = (c[0] - pitch_x / 2, c[1] - pitch_y / 2, c[0] + pitch_x / 2, c[1] + pitch_y / 2)
            out[i, j] = np.asarray(img.resize(size, Image.BILINEAR, box=box), dtype=np.float32) / 255
    return out, pitch_x, pitch_y


def shifted(p, dx, dy):
    return np.roll(np.roll(p, dy, axis=-2), dx, axis=-1)


def classify(patches, labels, classes, iters=4):
    """Assign every patch to the nearest class template, refining the templates."""
    flat = patches.reshape(-1, *patches.shape[2:])
    lab = labels.reshape(-1)
    assign = lab.copy()
    shifts = [(dx, dy) for dx in (-2, -1, 0, 1, 2) for dy in (-2, -1, 0, 1, 2)]
    for _ in range(iters):
        temps = {}
        for c in classes:
            sel = flat[assign == c]
            if len(sel) == 0:
                sel = flat[lab == c]
            temps[c] = np.median(sel, axis=0)
        names = list(temps)
        best = np.full(len(flat), np.inf)
        new = np.empty(len(flat), dtype=object)
        for c in names:
            d = np.min(
                [((shifted(flat, dx, dy) - temps[c]) ** 2).sum(axis=(1, 2)) for dx, dy in shifts],
                axis=0,
            )
            better = d < best
            best[better] = d[better]
            new[better] = c
        if (new == assign).all():
            break
        assign = new
    return assign.reshape(labels.shape)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("pdf")
    ap.add_argument("page", type=int)
    ap.add_argument("deck")
    ap.add_argument("--map", type=int, default=0, help="which map in the deck's output (from 0)")
    ap.add_argument("--bbox", help="approximate border: left,top,right,bottom")
    ap.add_argument("--corners", help="the corner characters' centres, x,y for top left, top right, "
                    "bottom left, bottom right; for maps whose fill hides the border lines")
    ap.add_argument("--rotate", type=int, default=-90, help="degrees anticlockwise to make the map upright")
    ap.add_argument("--search", type=int, default=80)
    ap.add_argument("--levels", default=".+OΘ█", help="the level symbols, lowest first")
    ap.add_argument("--symap", default="target/release/symap")
    ap.add_argument("--cache", default="target/scan-cache")
    ap.add_argument("--diff", help="write the cell-by-cell comparison here")
    ap.add_argument("--json", help="write the classified grid as JSON here")
    a = ap.parse_args()

    ours = symap_map(a.symap, a.deck, a.map)
    rows, cols = len(ours), len(ours[0])
    dark = normalise(render(a.pdf, a.page, a.rotate, a.cache))
    if a.corners:
        v = [float(x) for x in a.corners.split(",")]
        tl, tr, bl, br = (np.array(v[i:i + 2]) for i in range(0, 8, 2))
    else:
        tl, tr, bl, br = corners(dark, [int(v) for v in a.bbox.split(",")], a.search)
    patches, px, py = cells(dark, tl, tr, bl, br, rows, cols)
    print(f"grid {rows}x{cols}, pitch {px:.1f} x {py:.1f} px, corners "
          + " ".join(f"({c[0]:.0f},{c[1]:.0f})" for c in (tl, tr, bl, br)))

    # Classify the interior against the level symbols and blank; every other
    # character (digits, letters, legends) is left out.
    inner = (slice(1, rows - 1), slice(1, cols - 1))
    lab = np.array(ours, dtype=object)[inner]
    levels = list(a.levels)
    classes = [" "] + [c for c in levels if (lab == c).any()]
    keep = np.isin(lab, classes)
    sub = patches[inner][keep][:, None]
    got = np.full(lab.shape, None, dtype=object)
    got[keep] = classify(sub, lab[keep][:, None], classes)[:, 0]

    lvl = {c: i + 1 for i, c in enumerate(levels)}
    lvl[" "] = 0
    both = keep & np.vectorize(lambda c: c in lvl and c != " ")(lab)
    sym = both & np.vectorize(lambda c: c is not None and c != " ")(got)
    o = np.vectorize(lambda c: lvl.get(c, 0))(lab)
    g = np.vectorize(lambda c: lvl.get(c, 0) if c else 0)(got)
    print(f"characters compared: {keep.sum()}, agreeing: {(got[keep] == lab[keep]).mean():.2%}")
    if sym.sum():
        dl = np.abs(o[sym] - g[sym])
        print(f"level symbols on both: {sym.sum()}, same level {(dl == 0).mean():.2%}, "
              f"within one {(dl <= 1).mean():.2%}, scan higher {(g[sym] > o[sym]).sum()}, "
              f"lower {(g[sym] < o[sym]).sum()}")
    conf = Counter(zip(lab[keep], got[keep]))
    print("confusion (ours -> scan):")
    for (x, y), n in sorted(conf.items(), key=lambda t: -t[1]):
        if x != y:
            print(f"  {x!r} -> {y!r}: {n}")

    if a.diff:
        with open(a.diff, "w") as f:
            f.write("ours | scan | difference (scan's character where they differ)\n")
            for i in range(lab.shape[0]):
                orow = "".join(lab[i])
                srow = "".join(c if c else "?" for c in got[i])
                drow = "".join(
                    "·" if not keep[i, j] else ("_" if got[i, j] == lab[i, j] else got[i, j])
                    for j in range(lab.shape[1])
                )
                f.write(f"{i + 1:3d} {orow} | {srow} | {drow}\n")
    if a.json:
        with open(a.json, "w") as f:
            json.dump({"ours": ["".join(r) for r in lab],
                       "scan": ["".join(c if c else "?" for c in r) for r in got]}, f, ensure_ascii=False)


if __name__ == "__main__":
    sys.exit(main())
