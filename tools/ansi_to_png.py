#!/usr/bin/env python3
"""Renders 24-bit ANSI half-block terminal output (Figure::to_terminal with
TerminalMode::HalfBlock) to a PNG that looks like a terminal window.

    ansi_to_png.py terminal_output.txt out.png [title]
"""
import re
import sys
from PIL import Image, ImageDraw

src, dst = sys.argv[1], sys.argv[2]
title = sys.argv[3] if len(sys.argv) > 3 else "cargo run --release"
lines = [l for l in open(src, encoding="utf-8").read().split("\n") if "\x1b[" in l]
rows = []
for l in lines:
    if rows and len(rows) >= 25 and not l.startswith("\x1b[38"):
        break
    rows.append(l)
rows = rows[:25]
cw, ch, pad, bar = 8, 16, 24, 30
cols = max(len(re.sub(r"\x1b\[[0-9;]*m", "", l)) for l in rows)
W, H = cols * cw + 2 * pad, len(rows) * ch + 2 * pad + bar
img = Image.new("RGB", (W, H), (30, 30, 36))
d = ImageDraw.Draw(img)
d.rectangle([0, 0, W, bar], fill=(52, 52, 60))
for i, c in enumerate([(255, 95, 86), (255, 189, 46), (39, 201, 63)]):
    d.ellipse([12 + i * 22, 9, 24 + i * 22, 21], fill=c)
d.text((W // 2 - 60, 8), title, fill=(200, 200, 200))
for r, l in enumerate(rows):
    fg = bg = (30, 30, 36)
    x = i = 0
    while i < len(l):
        if l[i] == "\x1b":
            m = re.match(r"\x1b\[([0-9;]*)m", l[i:])
            i += len(m.group(0))
            p = m.group(1).split(";")
            k = 0
            while k < len(p):
                if p[k] == "38" and p[k + 1] == "2":
                    fg = tuple(map(int, p[k + 2:k + 5]))
                    k += 5
                elif p[k] == "48" and p[k + 1] == "2":
                    bg = tuple(map(int, p[k + 2:k + 5]))
                    k += 5
                else:
                    if p[k] in ("0", ""):
                        fg = bg = (30, 30, 36)
                    k += 1
            continue
        c = l[i]
        i += 1
        X, Y = pad + x * cw, pad + bar + r * ch
        if c == "▀":
            d.rectangle([X, Y, X + cw - 1, Y + ch // 2 - 1], fill=fg)
            d.rectangle([X, Y + ch // 2, X + cw - 1, Y + ch - 1], fill=bg)
        else:
            d.rectangle([X, Y, X + cw - 1, Y + ch - 1], fill=bg)
            if c.strip():
                d.text((X, Y + 1), c, fill=fg)
        x += 1
img.save(dst)
