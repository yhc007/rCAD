// WASM module loader utility using wasm-bindgen

import init, * as wasm from './pkg/rcad_wasm.js';

let initialized = false;

export interface CADDocument {
  create_box(width: number, height: number, depth: number): string;
  create_sphere(radius: number): string;
  create_cylinder(radius: number, height: number): string;
  create_cone(bottom_radius: number, top_radius: number, height: number): string;
  boolean_union(target_id: string, tool_id: string): string;
  boolean_subtract(target_id: string, tool_id: string): string;
  boolean_intersect(target_id: string, tool_id: string): string;
  tessellate(feature_id: string): any;
  undo(): boolean;
  redo(): boolean;
  export_stl(): Uint8Array;
  export_obj(): string;
}

export interface RenderContext {
  init(canvas: HTMLCanvasElement): Promise<void>;
  render(meshData: any): void;
  orbit(delta_x: number, delta_y: number): void;
  pan(delta_x: number, delta_y: number): void;
  zoom(delta: number): void;
}

export interface WasmModule {
  CADDocument: new () => CADDocument;
  RenderContext: new () => RenderContext;
  MeshData: any;
}

export async function loadWasm(): Promise<WasmModule> {
  if (!initialized) {
    await init();
    initialized = true;
    console.log('rCAD WASM module initialized');
  }

  return wasm as unknown as WasmModule;
}

export function isInitialized(): boolean {
  return initialized;
}

// Re-export types from the WASM module
export { wasm };

// Legacy utilities for direct memory access (if needed)
export function copyToWasm(
  memory: WebAssembly.Memory,
  data: Uint8Array,
  offset: number
): void {
  new Uint8Array(memory.buffer, offset, data.length).set(data);
}

export function copyFromWasm(
  memory: WebAssembly.Memory,
  offset: number,
  length: number
): Uint8Array {
  return new Uint8Array(memory.buffer, offset, length).slice();
}

// Utility for string handling
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

export function stringToWasm(
  memory: WebAssembly.Memory,
  str: string,
  offset: number
): number {
  const bytes = textEncoder.encode(str);
  copyToWasm(memory, bytes, offset);
  return bytes.length;
}

export function wasmToString(
  memory: WebAssembly.Memory,
  offset: number,
  length: number
): string {
  const bytes = copyFromWasm(memory, offset, length);
  return textDecoder.decode(bytes);
}
