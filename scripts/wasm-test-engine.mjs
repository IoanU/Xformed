// Run the shipped browser worker/client with Node's worker_threads and file-backed fetch.
import { Worker } from 'node:worker_threads';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';

export async function testEngine() {
  const script = await readFile(new URL('../dist/runtime.js', import.meta.url), 'utf8');
  const workers = new Set();
  class BrowserWorker {
    constructor(url) {
      const code = `
        const { parentPort, workerData } = require('node:worker_threads');
        const { readFile } = require('node:fs/promises');
        const originalFetch = globalThis.fetch;
        globalThis.fetch = async resource => {
          const url = new URL(resource);
          if (url.protocol !== 'file:') return originalFetch(resource);
          return new Response(await readFile(url), { headers: { 'Content-Type': 'application/wasm' } });
        };
        globalThis.self = { postMessage: data => parentPort.postMessage(data) };
        const ready = import(workerData.url);
        parentPort.on('message', async data => { await ready; self.onmessage({ data }); });
      `;
      this.native = new Worker(code, { eval: true, workerData: { url: String(url) } });
      workers.add(this);
      this.native.on('message', data => this.onmessage?.({ data }));
      this.native.on('error', error => this.onerror?.({ error, preventDefault() {} }));
    }
    postMessage(data) { this.native.postMessage(data); }
    terminate() { workers.delete(this); this.native.terminate(); }
  }
  const events = [];
  const window = { dispatchEvent: event => events.push(event.detail) };
  vm.runInNewContext(script, {
    document: { documentElement: { dataset: { engine: 'wasm' } }, currentScript: { src: String(new URL('../dist/runtime.js', import.meta.url)) } },
    window, Worker: BrowserWorker, URL, DOMException, CustomEvent, AbortSignal, setTimeout, clearTimeout
  });
  return { engine: window.XformedEngine, events, close() { for (const worker of workers) worker.terminate(); } };
}
