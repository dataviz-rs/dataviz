/** A chart description: `{ width?, height?, title?, theme?, panels: [{ type, ... }] }`
 * or one panel's fields with `type` at the top level. See `chartTypes()`. */
export type Spec = Record<string, unknown> | string;

export interface Hover {
  /** Index of the plot. */
  plot: number;
  /** Canvas position (CSS pixels) of the data point. */
  x: number;
  y: number;
  /** Series name and values. */
  text: string;
}

export interface Pixels {
  width: number;
  height: number;
  /** Straight-alpha RGBA (a view, valid until the next call). */
  data: Uint8ClampedArray;
}

/** Loads the WebAssembly module once (by default `dataviz.wasm` next to this file). */
export function init(source?: string | URL | BufferSource | WebAssembly.Module): Promise<void>;

export class Chart {
  constructor(spec: Spec);
  readonly handle: number;
  /** Canvas size in CSS pixels. */
  readonly width: number;
  readonly height: number;
  update(spec: Spec): this;
  resize(width: number, height: number): this;
  svg(): string;
  png(): Uint8Array;
  /** Self-contained interactive HTML page. */
  html(): string;
  pixels(scale?: number): Pixels;
  zoom(x: number, y: number, factor: number): boolean;
  pan(x: number, y: number, dx: number, dy: number): boolean;
  zoomBox(ax: number, ay: number, bx: number, by: number): boolean;
  reset(): void;
  click(x: number, y: number): boolean;
  hover(x: number, y: number): Hover | null;
  /** Draws on a canvas and handles zoom, pan, 3D turning, legend clicks and
   * hover readouts; returns a function removing the handlers. */
  mount(canvas: HTMLCanvasElement, options?: { interactive?: boolean; tooltip?: boolean }): () => void;
  free(): void;
}

export function chartTypes(): { type: string; fields: string; description: string }[];
export function svg(spec: Spec): string;
export function png(spec: Spec): Uint8Array;
