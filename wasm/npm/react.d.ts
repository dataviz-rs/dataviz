import type { Spec } from "./index.js";
export interface DatavizChartProps extends Record<string, unknown> {
  spec: Spec;
  interactive?: boolean;
  tooltip?: boolean;
  /** Where to load `dataviz.wasm` from (default: next to the package). */
  wasm?: string | URL;
}
export function DatavizChart(props: DatavizChartProps): unknown;
