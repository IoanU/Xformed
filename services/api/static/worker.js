// Each worker owns its WebAssembly memory. Termination cancels even synchronous Rust work.
import init, { convert_json } from './wasm/xformed_web.js';

try {
  await init();
  self.postMessage({ type: 'ready' });
} catch (error) {
  self.postMessage({ type: 'init-error', error: String(error?.message || error) });
}

self.onmessage = ({ data }) => {
  if (data.type !== 'convert') return;
  try {
    const result = JSON.parse(convert_json(JSON.stringify(data.request)));
    self.postMessage({ type: 'result', id: data.id, result });
  } catch (error) {
    self.postMessage({ type: 'error', id: data.id, error: String(error?.message || error) });
  }
};
