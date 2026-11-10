import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import { testEngine } from './wasm-test-engine.mjs';

const session = await testEngine();
const request = (from, to, payload, options = {}) => ({ from, to, payload, options: { target_seconds: 3, ...options } });
const text = { kind: 'Text', text: 'Rain gathers on the windows. Somewhere, a city is learning to be quiet.' };
const image = { kind: 'ImageBase64', data_b64: (await readFile(new URL('../inputs/images.png', import.meta.url))).toString('base64') };
const decode = artifact => Buffer.from(artifact.data_b64, 'base64');
let wav, musicalResult;
try {
  await session.engine.ready();
  for (const [from, payload, targets] of [['text', text, ['image', 'audio', 'json']], ['image', image, ['text', 'audio', 'json']]]) {
    for (const to of targets) {
      const result = await session.engine.convert(request(from, to, payload));
      if (to === 'image') {
        const png = decode(result.artifacts.find(a => a.type === 'PngBase64'));
        assert.equal(png.subarray(1, 4).toString(), 'PNG'); assert.equal(png.readUInt32BE(16), 1280);
      } else if (to === 'audio') {
        wav = decode(result.artifacts.find(a => a.type === 'WavBase64'));
        assert.equal(wav.toString('ascii', 0, 4), 'RIFF'); assert.equal(wav.readUInt16LE(22), 2);
        const score = JSON.parse(decode(result.artifacts.find(a => a.type === 'MidiJsonBase64')));
        const midi = decode(result.artifacts.find(a => a.type === 'MidiBase64'));
        assert.equal(midi.toString('ascii', 0, 4), 'MThd'); assert.equal(midi.readUInt16BE(10), score.tracks.length + 1);
        assert.ok(midi.includes(Buffer.from([255, 88, 4, score.meter[0], Math.log2(score.meter[1])])));
        if (from === 'text') musicalResult = result;
      } else if (to === 'text') assert.ok(result.artifacts.find(a => a.type === 'Text').text.length > 100);
      else assert.ok(result.artifacts.some(a => a.type === 'Json'));
      console.log(`PASS wasm worker: ${from} -> ${to}`);
    }
  }
  for (const to of ['image', 'text', 'json']) {
    const result = await session.engine.convert(request('audio', to, { kind: 'AudioBase64', data_b64: wav.toString('base64') }));
    assert.ok(result.artifacts.some(a => a.type === ({ image: 'PngBase64', text: 'Text', json: 'Json' })[to]));
    console.log(`PASS wasm worker: audio -> ${to}`);
  }
  await assert.rejects(session.engine.convert(request('text', 'image', { kind: 'Text', text: '' })));
  const repeated = await session.engine.convert(request('text', 'audio', text));
  assert.equal(repeated.artifacts.find(a => a.type === 'MidiJsonBase64').data_b64, musicalResult.artifacts.find(a => a.type === 'MidiJsonBase64').data_b64);
  const varied = await session.engine.convert(request('text', 'audio', text, { variation: 1 }));
  assert.notEqual(varied.artifacts.find(a => a.type === 'MidiJsonBase64').data_b64, musicalResult.artifacts.find(a => a.type === 'MidiJsonBase64').data_b64);
  const cancel = new AbortController();
  const long = session.engine.convert(request('text', 'audio', text, { target_seconds: 90 }), { signal: cancel.signal });
  const rejected = assert.rejects(long, error => error.name === 'AbortError');
  await new Promise(resolve => setTimeout(resolve, 50));
  const start = performance.now(); cancel.abort(); await rejected;
  assert.ok(performance.now() - start < 1500, 'Cancel must terminate work promptly');
  const recovered = await session.engine.convert(request('text', 'json', text));
  assert.ok(recovered.artifacts.some(a => a.type === 'Json'));
  console.log('PASS: error recovery, repeatability, musical takes, cancellation and worker restart');
} finally { session.close(); }

const startup = await testEngine();
try {
  const controller = new AbortController();
  const first = startup.engine.convert(request('text', 'json', text), { signal: controller.signal });
  const rejected = assert.rejects(first, error => error.name === 'AbortError');
  await assert.rejects(startup.engine.convert(request('text', 'json', text)), /already running/);
  controller.abort(); await rejected;
  await startup.engine.convert(request('text', 'json', text));
  console.log('PASS: cancellation during initialization and concurrent request protection');
} finally { startup.close(); }
console.log(`WebAssembly binary: ${((await stat(new URL('../dist/wasm/xformed_web_bg.wasm', import.meta.url))).size / 1024 / 1024).toFixed(2)} MiB`);
