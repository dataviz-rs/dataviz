#!/usr/bin/env python3
"""Converts Natural Earth's 1:110m admin-0 countries (public domain) to the
compact binary `assets/world_110m.bin` used by `dataviz::plot::geo::world`.

    curl -sLO https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_110m_admin_0_countries.geojson
    python3 tools/gen_world.py ne_110m_admin_0_countries.geojson assets/world_110m.bin

Format (little endian): b"DVW1", u16 country count; per country: name,
long name, ISO A2, ISO A3, continent (u8 length + UTF-8), f64 population,
f64 GDP (million USD), u16 polygon count; per polygon: u16 ring count; per
ring: u16 point count, then (i16 lon, i16 lat) in 1/100 degree.
"""
import json
import struct
import sys

src, dst = sys.argv[1], sys.argv[2]
features = json.load(open(src, encoding="utf-8"))["features"]


def text(s):
    b = (s or "").encode("utf-8")[:255]
    return struct.pack("<B", len(b)) + b


def code(p, *keys):
    for k in keys:
        v = p.get(k)
        if v and v != "-99":
            return v
    return ""


out = bytearray(b"DVW1")
out += struct.pack("<H", len(features))
for f in sorted(features, key=lambda f: f["properties"]["NAME"]):
    p = f["properties"]
    g = f["geometry"]
    polys = [g["coordinates"]] if g["type"] == "Polygon" else g["coordinates"]
    out += text(p["NAME"]) + text(p.get("NAME_LONG")) + text(code(p, "ISO_A2_EH", "ISO_A2"))
    out += text(code(p, "ISO_A3_EH", "ISO_A3", "ADM0_A3")) + text(p.get("CONTINENT"))
    out += struct.pack("<dd", float(p.get("POP_EST") or 0), float(p.get("GDP_MD") or 0))
    out += struct.pack("<H", len(polys))
    for poly in polys:
        out += struct.pack("<H", len(poly))
        for ring in poly:
            out += struct.pack("<H", len(ring))
            for lon, lat in ring:
                out += struct.pack("<hh", round(lon * 100), round(lat * 100))
open(dst, "wb").write(out)
print(f"{len(features)} countries, {len(out)} bytes")
