import type { Spec } from "./index.js";
export function dataviz(canvas: HTMLCanvasElement, spec: Spec): {
  update(spec: Spec): void;
  destroy(): void;
};
