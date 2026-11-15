#!/usr/bin/env python3
"""Builds README.md from tools/README.template.md: {{PERFORMANCE}} from
perf_section.py (dataviz-examples/benchmarks/results.json) and {{GALLERY}}
from gallery.py (dataviz-examples screenshots).

    python3 tools/readme.py          # writes README.md
    python3 tools/readme.py --check  # fails when README.md is out of date
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)


def run(script):
    return subprocess.run([sys.executable, os.path.join(HERE, script)], check=True,
                          capture_output=True, text=True).stdout.rstrip("\n")


text = open(os.path.join(HERE, "README.template.md")).read()
text = text.replace("{{PERFORMANCE}}", run("perf_section.py")).replace("{{GALLERY}}", run("gallery.py"))
path = os.path.join(ROOT, "README.md")
if "--check" in sys.argv:
    if open(path).read() != text:
        sys.exit("README.md is out of date: run python3 tools/readme.py")
else:
    open(path, "w").write(text)
