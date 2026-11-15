# Output

```rust,ignore
fig.save("chart.png")?;            // .svg .pdf .jpg .webp .bmp .html
let bytes = fig.encode(Format::Png)?;
fig.render_into(&mut pixels)?;     // RGBA, reusing the buffer
fig.print()?;                      // terminal: colored half blocks or Braille
fig.record("wave.gif", 30.0, 120, |fig, i| { /* change the figure */ })?;  // .gif .png (APNG) .mp4 .webm
let text = fig.alt_text();         // a description for screen readers and captions
```

## Backends

```rust,ignore
fig.backend(Backend::Auto);                // CPU, all cores from 1 megapixel on
fig.backend(Backend::Cpu { threads: 1 });  // least CPU time and memory
fig.backend(Backend::Gpu);                 // feature "gpu"
```

## Accessibility

SVG, PDF and HTML carry the title and a generated description;
`fig.color_warnings()` finds series that look alike with color vision
deficiencies and `pixels.simulate(Cvd::Deuteranopia)` shows the chart as
others see it.
