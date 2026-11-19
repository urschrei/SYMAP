// Loader for the SYMAP WebAssembly module. The bindings in pkg/ are made
// by wasm-bindgen (see scripts/build-web.sh and src/wasm.rs).
import init, { run, importData } from './pkg/symap.js';

/**
 * Load the module. `run(text)` runs a deck; `importData(request)` makes a
 * deck from data (see json::ImportRequest). Both return plain objects.
 */
export async function loadSymap() {
  await init();
  return { run, importData };
}

/**
 * A SYMAP runner that works in a Web Worker, keeping at most one run in
 * flight: while one runs, only the latest request waits, so typing never
 * queues up stale work. Falls back to the main thread without workers.
 */
export async function createRunner() {
  let worker;
  try {
    worker = new Worker(new URL('worker.js', import.meta.url), { type: 'module' });
  } catch {
    const symap = await loadSymap();
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
