// WASM module loader utility

let wasmInstance: WebAssembly.Instance | null = null;
let wasmModule: WebAssembly.Module | null = null;

export interface WasmExports {
  memory: WebAssembly.Memory;
  // Add other exports as needed
}

export async function loadWasm(
  wasmUrl: string
): Promise<{ instance: WebAssembly.Instance; module: WebAssembly.Module }> {
  if (wasmInstance && wasmModule) {
    return { instance: wasmInstance, module: wasmModule };
  }

  const response = await fetch(wasmUrl);
  const buffer = await response.arrayBuffer();

  const result = await WebAssembly.instantiate(buffer, {
    env: {
      // Environment functions the WASM module might need
      abort: () => {
        throw new Error('WASM abort called');
      },
    },
    wbg: {
      // wasm-bindgen glue code imports
    },
  });

  wasmInstance = result.instance;
  wasmModule = result.module;

  return result;
}

export function getWasmExports(): WasmExports | null {
  if (!wasmInstance) return null;
  return wasmInstance.exports as unknown as WasmExports;
}

// Utility to copy data to/from WASM memory
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
