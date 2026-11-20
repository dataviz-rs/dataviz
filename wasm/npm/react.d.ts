import type { Hover, Spec } from "./index.js";
export interface DatavizChartProps extends Record<string, unknown> {
  spec: Spec;
  interactive?: boolean;
  tooltip?: boolean;
  /** Where to load `dataviz.wasm` from (default: next to the package). */
  wasm?: string | URL;
  /** A click on the data: the nearest point (null when none). */
  onPointClick?: (point: Hover | null) => void;
  /** The point under the mouse changed. */
  onPointHover?: (point: Hover | null) => void;
  /** After zooming or panning. */
  onView?: () => void;
}
export function DatavizChart(props: DatavizChartProps): unknown;
