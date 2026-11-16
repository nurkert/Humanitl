#!/usr/bin/env python3
"""Rasterise the fist of the logo for the boot animation of `humanitl go` (HUM-496).

The fist in `packaging/deb/humanitl.svg` is four rounded rectangles (the
fingers) and one path (thumb and back of the hand), in the coordinate space
of the Font Awesome glyph, 448 x 512 units. This script samples that shape on
a coarse grid and writes the coverage of every cell as one digit, 0 (empty)
to 9 (full), into `daemon/bin/humanitl/src/splash/fist.rs`. The renderer
extrudes this silhouette into a slab and uses the coverage as a smooth
density, so the edges get a bevel instead of stairs.

Only the standard library; run it again when the logo changes:

    python3 tools/splash_fist.py

`--check` compares instead of writing and exits 1 on a difference.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SVG = ROOT / "packaging/deb/humanitl.svg"
OUT = ROOT / "daemon/bin/humanitl/src/splash/fist.rs"

# Grid of the bitmap: 8 glyph units per cell, 4 x 4 samples per cell.
X0, X1 = 24.0, 424.0
Y0, Y1 = -8.0, 520.0
CELL = 8.0
SUB = 4


def flatten_path(d: str) -> list[tuple[float, float]]:
    """Turns an SVG path (M, L, l, C, c, S, s, z) into a polygon."""
    tokens = re.findall(r"[MLCSZmlcsz]|-?(?:\d+\.?\d*|\.\d+)", d)
    points: list[tuple[float, float]] = []
    x = y = 0.0
    last_ctrl: tuple[float, float] | None = None
    i = 0
    cmd = ""

    def num() -> float:
        nonlocal i
        value = float(tokens[i])
        i += 1
        return value

    def cubic(p0, p1, p2, p3, steps=12):
        for k in range(1, steps + 1):
            t = k / steps
            u = 1 - t
            px = u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0]
            py = u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1]
            points.append((px, py))

    while i < len(tokens):
        if re.fullmatch(r"[A-Za-z]", tokens[i]):
            cmd = tokens[i]
            i += 1
            if cmd in "Zz":
                last_ctrl = None
                continue
        if cmd == "M":
            x, y = num(), num()
            points.append((x, y))
            cmd = "L"
            last_ctrl = None
        elif cmd == "L":
            x, y = num(), num()
            points.append((x, y))
            last_ctrl = None
        elif cmd == "l":
            x, y = x + num(), y + num()
            points.append((x, y))
            last_ctrl = None
        elif cmd in "Cc":
            rel = cmd == "c"
            c1 = (num(), num())
            c2 = (num(), num())
            end = (num(), num())
            if rel:
                c1 = (x + c1[0], y + c1[1])
                c2 = (x + c2[0], y + c2[1])
                end = (x + end[0], y + end[1])
            cubic((x, y), c1, c2, end)
            last_ctrl = c2
            x, y = end
        elif cmd in "Ss":
            rel = cmd == "s"
            c1 = (2 * x - last_ctrl[0], 2 * y - last_ctrl[1]) if last_ctrl else (x, y)
            c2 = (num(), num())
            end = (num(), num())
            if rel:
                c2 = (x + c2[0], y + c2[1])
                end = (x + end[0], y + end[1])
            cubic((x, y), c1, c2, end)
            last_ctrl = c2
            x, y = end
        else:
            raise ValueError(f"unsupported path command {cmd!r}")
    return points


def inside_polygon(px: float, py: float, poly: list[tuple[float, float]]) -> bool:
    """Even-odd test; the path is one simple contour, so it equals nonzero."""
    inside = False
    n = len(poly)
    for k in range(n):
        ax, ay = poly[k]
        bx, by = poly[(k + 1) % n]
        if (ay > py) != (by > py):
            cross = ax + (py - ay) * (bx - ax) / (by - ay)
            if px < cross:
                inside = not inside
    return inside


def inside_rounded_rect(px, py, rect) -> bool:
    x, y, w, h, r = rect
    if not (x <= px <= x + w and y <= py <= y + h):
        return False
    cx = min(max(px, x + r), x + w - r)
    cy = min(max(py, y + r), y + h - r)
    return (px - cx) ** 2 + (py - cy) ** 2 <= r * r


def shape_of(svg: str):
    group = svg.split('fill="#14161C">', 1)[1].split("</g>", 1)[0]
    rects = [
        tuple(float(v) for v in m)
        for m in re.findall(
            r'<rect x="([\d.]+)" y="([\d.]+)" width="([\d.]+)" height="([\d.]+)" rx="([\d.]+)"',
            group,
        )
    ]
    path = re.search(r'<path d="([^"]+)"', group).group(1)
    return rects, flatten_path(path)


def render(svg: str) -> str:
    rects, poly = shape_of(svg)
    cols = int((X1 - X0) / CELL)
    rows = int((Y1 - Y0) / CELL)
    lines = []
    for row in range(rows):
        line = []
        for col in range(cols):
            hits = 0
            for sy in range(SUB):
                for sx in range(SUB):
                    px = X0 + (col + (sx + 0.5) / SUB) * CELL
                    py = Y0 + (row + (sy + 0.5) / SUB) * CELL
                    if any(inside_rounded_rect(px, py, r) for r in rects) or inside_polygon(
                        px, py, poly
                    ):
                        hits += 1
            line.append(str(round(9 * hits / (SUB * SUB))))
        lines.append("".join(line))
    body = "\n".join(f'    "{line}",' for line in lines)
    return (
        "//! Die Silhouette der Faust aus dem Logo, als Bedeckung je Zelle (HUM-496).\n"
        "//!\n"
        "//! Erzeugt von `tools/splash_fist.py` aus `packaging/deb/humanitl.svg`; nicht\n"
        "//! von Hand ändern. Jede Ziffer ist der Anteil der Zelle, den die Faust\n"
        "//! bedeckt, von 0 (leer) bis 9 (voll); Zeile 0 ist oben.\n\n"
        f"/// Breite der Silhouette in Zellen.\npub const WIDTH: usize = {cols};\n\n"
        f"/// Höhe der Silhouette in Zellen.\npub const HEIGHT: usize = {rows};\n\n"
        "/// Die Zeilen der Silhouette, oben zuerst.\n"
        f"pub const ROWS: [&str; HEIGHT] = [\n{body}\n];\n"
    )


def main() -> int:
    text = render(SVG.read_text(encoding="utf-8"))
    if "--check" in sys.argv[1:]:
        if OUT.read_text(encoding="utf-8") != text:
            print(f"{OUT.relative_to(ROOT)} is stale; run python3 tools/splash_fist.py")
            return 1
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
