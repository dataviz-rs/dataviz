// React component: <DatavizChart spec={...} />. React is a peer dependency
// (only this entry point imports it).
import { createElement, useEffect, useRef } from "react";
import { init, Chart } from "./index.js";

/**
 * A live dataviz chart on a canvas: zoom, pan, 3D turning, legend clicks
 * and hover readouts. A new `spec` updates the chart in place.
 * `onPointClick(point)`, `onPointHover(point)` and `onView()` report clicks
 * on the data, the point under the mouse and zooming or panning.
 */
export function DatavizChart({
  spec,
  interactive = true,
  tooltip = true,
  wasm,
  onPointClick,
  onPointHover,
  onView,
  ...props
}) {
  const ref = useRef(null);
  const chart = useRef(null);
  // The latest callbacks, without rebuilding the chart when they change.
  const cb = useRef({});
  cb.current = { onPointClick, onPointHover, onView };
  useEffect(() => {
    let off = null;
    let live = true;
    init(wasm).then(() => {
      if (!live || !ref.current) return;
      chart.current = new Chart(spec);
      off = chart.current.mount(ref.current, {
        interactive,
        tooltip,
        onClick: (p) => cb.current.onPointClick?.(p),
        onHover: (p) => cb.current.onPointHover?.(p),
        onView: () => cb.current.onView?.(),
      });
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
