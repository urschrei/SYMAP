// Loader for symap.wasm: a dependency-free module exporting
// symap_alloc / symap_run / symap_result_ptr (see src/wasm.rs).

export async function loadSymap(url) {
  let instance;
  try {
    ({ instance } = await WebAssembly.instantiateStreaming(fetch(url), {}));
  } catch {
    // Servers that send the wrong MIME type for .wasm.
    const bytes = await (await fetch(url)).arrayBuffer();
    ({ instance } = await WebAssembly.instantiate(bytes, {}));
  }
  const w = instance.exports;
  const enc = new TextEncoder();
  const dec = new TextDecoder();
  return {
    /** Run a deck; returns the parsed JSON result. */
    run(text) {
      const input = enc.encode(text);
      const ptr = w.symap_alloc(input.length);
      new Uint8Array(w.memory.buffer, ptr, input.length).set(input);
      const n = w.symap_run(ptr, input.length);
      const out = new Uint8Array(w.memory.buffer, w.symap_result_ptr(), n);
      return JSON.parse(dec.decode(out));
    },
  };
}
