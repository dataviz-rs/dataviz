#!/usr/bin/env python3
"""Generates the chart catalog and performance pages of the guide (book/)
from the example READMEs and the benchmark results, and writes
book/src/SUMMARY.md.

    python3 tools/book.py [path/to/dataviz-examples]
"""
import importlib.util
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
EX = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "..", "dataviz-examples")
SRC = os.path.join(ROOT, "book", "src")
RAW = "https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main"
TREE = "https://github.com/dataviz-rs/dataviz-examples/tree/main"

spec = importlib.util.spec_from_file_location("gallery", os.path.join(HERE, "gallery.py"))
src = open(os.path.join(HERE, "gallery.py")).read()
# The SECTIONS list only (the gallery script prints when run).
ns = {}
exec(src.split("\nfor name, items in SECTIONS:")[0], ns)
SECTIONS = ns["SECTIONS"]


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")


def example_page(name):
    """The README of an example as a catalog entry: absolute image links,
    without the Run / License sections."""
    path = os.path.join(EX, name, "README.md")
    if not os.path.exists(path):
        return None
    text = open(path, encoding="utf-8").read()
    text = re.split(r"\n## (Run|License)\b", text)[0]
    text = re.sub(r"Built with the \[`dataviz::plot`\]\([^)]*\) API\.\n?", "", text)
    text = text.replace('src="./', f'src="{RAW}/{name}/').replace("](./", f"]({RAW}/{name}/")
    text = re.sub(r"\]\((src/[^)]*)\)", rf"]({TREE}/{name}/\1)", text)
    # Headings one level down inside the chapter.
    text = re.sub(r"^(#+) ", lambda m: "#" + m.group(1) + " ", text, flags=re.M)
    return text.strip() + f"\n\n[Code of this example]({TREE}/{name}) · `cargo run --release -p {name}`\n"


os.makedirs(os.path.join(SRC, "catalog"), exist_ok=True)
chapters = []
for title, items in SECTIONS:
    names = []
    for name, _, _ in items:
        if name not in names:
            names.append(name)
    pages = [p for p in (example_page(n) for n in names) if p]
    if not pages:
        continue
    file = f"catalog/{slug(title)}.md"
    open(os.path.join(SRC, file), "w", encoding="utf-8").write(f"# {title}\n\n" + "\n---\n\n".join(pages))
    chapters.append((title, file))

index = ["# Chart catalog\n", "Every chart type with a picture and its code, grouped like the gallery.", ""]
index += [f"- [{t}]({os.path.basename(f)})" for t, f in chapters]
open(os.path.join(SRC, "catalog", "index.md"), "w").write("\n".join(index) + "\n")

perf = subprocess.run([sys.executable, os.path.join(HERE, "perf_section.py"), os.path.join(EX, "benchmarks", "results.json")],
                      capture_output=True, text=True, check=True).stdout
open(os.path.join(SRC, "performance.md"), "w").write(perf.replace("## Performance", "# Performance", 1))

summary = ["# Summary", "", "[Introduction](introduction.md)", "",
           "- [Installation](install.md)", "- [Quick start](quickstart.md)", "- [Your data](data.md)",
           "- [Choosing a chart](choosing.md)", "- [Chart catalog](catalog/index.md)"]
summary += [f"  - [{t}]({f})" for t, f in chapters]
summary += ["- [Interactivity](interactive.md)", "- [3D](three_d.md)", "- [Maps](maps.md)", "- [Output](output.md)",
            "- [Performance](performance.md)", "- [Python](python.md)", "- [Claude and MCP](claude.md)"]
open(os.path.join(SRC, "SUMMARY.md"), "w").write("\n".join(summary) + "\n")
print(f"{len(chapters)} catalog chapters")
