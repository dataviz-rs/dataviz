// Svelte action (works with any framework that has element actions):
//   <canvas use:dataviz={spec} on:pointclick={e => e.detail} />
// Events on the canvas: `pointclick` and `pointhover` (detail: the nearest
// point or null) and `view` (after zooming or panning).
import { init, Chart } from "./index.js";

/** Draws `spec` on the canvas and keeps it interactive; updates in place. */
export function dataviz(canvas, spec) {
  let chart = null;
  let off = null;
  let next = spec;
  let gone = false;
  init().then(() => {
    if (gone) return;
    chart = new Chart(next);
    const fire = (type) => (detail) =>
      canvas.dispatchEvent(new CustomEvent(type, { detail }));
    off = chart.mount(canvas, {
      onClick: fire("pointclick"),
      onHover: fire("pointhover"),
      onView: fire("view"),
    });
  });
  return {
    update(s) {
      next = s;
      chart?.update(s);
    },
    destroy() {
      gone = true;
      off?.();
      chart?.free();
    },
  };
}
