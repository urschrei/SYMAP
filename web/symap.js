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

/**
 * A SYMAP runner that works in a Web Worker, keeping at most one run in
 * flight: while one runs, only the latest request waits, so typing never
 * queues up stale work. Falls back to the main thread without workers.
 */
export async function createRunner(wasmUrl) {
  let worker;
  try {
    worker = new Worker(new URL('worker.js', import.meta.url), { type: 'module' });
  } catch {
    const symap = await loadSymap(wasmUrl);
    return { run: async (text) => symap.run(text) };
  }
  let nextId = 0;
  let busy = null;      // {id, resolve, reject}
  let waiting = null;   // {text, resolve, reject}
  const start = (job) => {
    busy = { id: ++nextId, ...job };
    worker.postMessage({ id: busy.id, text: job.text });
  };
  worker.onmessage = ({ data }) => {
    const done = busy;
    busy = null;
    if (waiting) {
      const job = waiting;
      waiting = null;
      start(job);
    }
    if (data.failure) done.reject(new Error(data.failure));
    else done.resolve(data.result);
  };
  worker.onerror = (e) => {
    const err = new Error(e.message || 'SYMAP worker failed');
    busy?.reject(err);
    waiting?.reject(err);
    busy = waiting = null;
  };
  return {
    /** Resolves with the result, or with null if a newer request superseded it. */
    run(text) {
      return new Promise((resolve, reject) => {
        if (!busy) {
          start({ text, resolve, reject });
        } else {
          waiting?.resolve(null);
          waiting = { text, resolve, reject };
        }
      });
    },
  };
}
