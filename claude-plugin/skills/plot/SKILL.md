---
name: plot
description: Draw a chart of a data file or of data described in the request with dataviz.
argument-hint: "<file path or what to plot>"
disable-model-invocation: true
---

Make a chart for: $ARGUMENTS

- If it names a CSV, TSV or JSON file, call the dataviz `plot_file` tool
  (kind auto first; choose columns and kind from the file's header when the
  request says what to show).
- Otherwise gather the data (from the conversation, the code or by
  computing it) and call the dataviz `plot` tool; call `chart_types` if
  unsure which panel type fits.
- Look at the image, improve it once if labels, ranges or the chart type
  are off, then show it and summarize what it shows in one or two
  sentences. Save it with `output` only if the request asks to save.
