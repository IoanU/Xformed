/* One interface for the browser worker and the optional native HTTP server. */
(() => {
  'use strict';
  const browserEngine = document.documentElement.dataset.engine === 'wasm';
  const base = new URL('.', document.currentScript.src);
  let worker = null, boot = null, bootControl = null, pending = null, nextId = 0;
  const abortError = () => new DOMException('Transformation canceled', 'AbortError');
  const state = (value) => window.dispatchEvent(new CustomEvent('xformed:engine', { detail: value }));

  function dispose(error) {
    const previous = worker;
    worker = null;
    boot = null;
    if (bootControl) {
      clearTimeout(bootControl.timer);
      bootControl.reject(error);
      bootControl = null;
    }
    previous?.terminate();
    if (pending) {
      const operation = pending;
      pending = null;
      operation.cleanup();
      operation.reject(error);
    }
  }

  function initialize() {
    if (boot) return boot;
    state('loading');
    let resolveBoot, rejectBoot;
    boot = new Promise((resolve, reject) => { resolveBoot = resolve; rejectBoot = reject; });
    // Prevent an abandoned initialization rejection after a canceled conversion.
    boot.catch(() => {});
    let instance;
    try {
      instance = new Worker(new URL('worker.js', base), { type: 'module' });
      worker = instance;
    } catch (error) {
      boot = null;
      rejectBoot(error);
      state('error');
      return Promise.reject(error);
    }
    const timeout = setTimeout(() => fail(new Error('The engine could not load. Reload this page to retry.')), 60000);
    bootControl = { timer: timeout, reject: rejectBoot };
    function fail(error) {
      if (worker !== instance) return;
      clearTimeout(timeout);
      rejectBoot(error);
      dispose(error);
      state('error');
    }
    instance.onerror = (event) => {
      event.preventDefault();
      fail(new Error('The renderer stopped. Try a shorter recording or a smaller image.'));
    };
    instance.onmessage = ({ data }) => {
      if (worker !== instance) return;
      if (data.type === 'ready') {
        clearTimeout(timeout);
        bootControl = null;
        resolveBoot();
        state('ready');
      } else if (data.type === 'init-error') {
        fail(new Error('The engine could not load. Reload this page to retry.'));
      } else if (pending?.id === data.id) {
        const operation = pending;
        pending = null;
        operation.cleanup();
        if (data.type === 'result') operation.resolve(data.result);
        else operation.reject(new Error(data.error || 'This source could not be converted.'));
      }
    };
    return boot;
  }

  async function ready() {
    if (browserEngine) return initialize();
    const response = await fetch(new URL('health', base), { signal: AbortSignal.timeout(3000) });
    if (!response.ok) throw new Error('The local studio is offline. Start Xformed Studio, then try again.');
  }

  async function convert(request, { signal } = {}) {
    if (signal?.aborted) throw abortError();
    if (!browserEngine) {
      const response = await fetch(new URL('convert', base), {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request), signal
      });
      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        throw new Error(body.error || `The source could not be transformed (${response.status}).`);
      }
      return response.json();
    }
    if (pending) throw new Error('A transformation is already running.');
    // Cancellation also covers the initial download/compilation of the module.
    return new Promise((resolve, reject) => {
      const id = ++nextId;
      let settled = false;
      const cleanup = () => { settled = true; signal?.removeEventListener('abort', cancel); };
      const cancel = () => {
        if (settled) return;
        dispose(abortError());
        state('idle');
      };
      signal?.addEventListener('abort', cancel, { once: true });
      pending = { id, resolve, reject, cleanup };
      initialize().then(() => {
        if (settled) return;
        if (signal?.aborted) { cancel(); return; }
        worker.postMessage({ type: 'convert', id, request });
      }).catch(error => {
        if (!settled) {
          if (pending?.id === id) pending = null;
          cleanup(); reject(error);
        }
      });
    });
  }
  window.XformedEngine = { browser: browserEngine, ready, convert };
})();
