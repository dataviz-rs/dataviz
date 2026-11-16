// React component: <DatavizChart spec={...} />. React is a peer dependency
// (only this entry point imports it).
import { createElement, useEffect, useRef } from "react";
import { init, Chart } from "./index.js";

/**
 * A live dataviz chart on a canvas: zoom, pan, 3D turning, legend clicks
 * and hover readouts. A new `spec` updates the chart in place.
 */
export function DatavizChart({ spec, interactive = true, tooltip = true, wasm, ...props }) {
  const ref = useRef(null);
  const chart = useRef(null);
  useEffect(() => {
    let off = null;
    let live = true;
    init(wasm).then(() => {
      if (!live || !ref.current) return;
      chart.current = new Chart(spec);
      off = chart.current.mount(ref.current, { interactive, tooltip });
    });
    return () => {
      live = false;
      off?.();
      chart.current?.free();
      chart.current = null;
    };
    // The chart is rebuilt only when the interaction mode changes.
  }, [interactive, tooltip, wasm]);
  useEffect(() => {
    chart.current?.update(spec);
  }, [spec]);
  return createElement("canvas", { ref, ...props });
}
