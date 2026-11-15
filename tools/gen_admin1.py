#!/usr/bin/env python3
"""Converts Natural Earth's 1:10m admin-1 states and provinces (public
domain) to the compressed binary `assets/admin1.bin` used by
`dataviz::plot::geo::regions`.

    curl -sLO https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_admin_1_states_provinces.geojson
    python3 tools/gen_admin1.py ne_10m_admin_1_states_provinces.geojson assets/admin1.bin

Rings are simplified (Douglas–Peucker, 0.02°) and quantized to 1/100
degree. Format: zlib of (little endian) b"DVA1", u16 region count; per
region: name, ISO 3166-2 code, country ISO A3, type (u8 length + UTF-8),
u16 polygon count; per polygon: u16 ring count; per ring: u16 point count,
then (i16 lon, i16 lat) in 1/100 degree.
"""
import json
import struct
import sys
import zlib

TOL = 0.02


def simplify(pts, tol):
    if len(pts) < 4:
        return pts
    keep = [False] * len(pts)
    keep[0] = keep[-1] = True
    stack = [(0, len(pts) - 1)]
    while stack:
        a, b = stack.pop()
        (x1, y1), (x2, y2) = pts[a], pts[b]
        dx, dy = x2 - x1, y2 - y1
        norm = (dx * dx + dy * dy) ** 0.5
        best, idx = -1.0, -1
        for i in range(a + 1, b):
            x, y = pts[i]
            d = abs(dy * (x - x1) - dx * (y - y1)) / norm if norm > 0 else ((x - x1) ** 2 + (y - y1) ** 2) ** 0.5
            if d > best:
                best, idx = d, i
        if best > tol:
            keep[idx] = True
            stack += [(a, idx), (idx, b)]
    return [p for p, k in zip(pts, keep) if k]


def text(s):
    b = (s or "").encode("utf-8")[:255]
    return struct.pack("<B", len(b)) + b


src, dst = sys.argv[1], sys.argv[2]
features = json.load(open(src, encoding="utf-8"))["features"]
out = bytearray(b"DVA1")
regions = []
for f in features:
    p, g = f["properties"], f["geometry"]
    if not g:
        continue
    polys = [g["coordinates"]] if g["type"] == "Polygon" else g["coordinates"]
    enc = []
    for poly in polys:
        rings = []
        for ring in poly:
            q = []
            for lon, lat in simplify([tuple(c[:2]) for c in ring], TOL):
                pt = (round(lon * 100), round(lat * 100))
                if not q or q[-1] != pt:
                    q.append(pt)
            if len(q) >= 4:
                rings.append(q)
        if rings:
            enc.append(rings)
    if enc:
        regions.append((p, enc))
regions.sort(key=lambda r: (r[0].get("adm0_a3") or "", r[0].get("name") or ""))
out += struct.pack("<H", len(regions))
for p, polys in regions:
    out += text(p.get("name_en") or p.get("name")) + text(p.get("iso_3166_2")) + text(p.get("adm0_a3")) + text(p.get("type_en"))
    out += struct.pack("<H", len(polys))
    for rings in polys:
        out += struct.pack("<H", len(rings))
        for q in rings:
            out += struct.pack("<H", len(q))
            for pt in q:
                out += struct.pack("<hh", *pt)
z = zlib.compress(bytes(out), 9)
open(dst, "wb").write(z)
print(f"{len(regions)} regions, {len(out)} bytes, {len(z)} compressed")
