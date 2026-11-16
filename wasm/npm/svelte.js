// Svelte action (works with any framework that has element actions):
//   <canvas use:dataviz={spec} />
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
    off = chart.mount(canvas);
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
