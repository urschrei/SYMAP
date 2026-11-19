// Runs SYMAP off the main thread. Messages: {id, text} -> {id, result}.
import { loadSymap } from './symap.js';

const ready = loadSymap();

self.onmessage = async ({ data }) => {
  try {
    const symap = await ready;
    self.postMessage({ id: data.id, result: symap.run(data.text) });
  } catch (err) {
    self.postMessage({ id: data.id, failure: String(err && err.message || err) });
  }
};
