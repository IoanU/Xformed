/* Persistent source/destination choreography. Anime.js is bundled locally. */
(() => {
  'use strict';
  const canvas = document.getElementById('portal');
  const ctx = canvas.getContext('2d');
  const routeMap = document.getElementById('route-map');
  const routePath = document.getElementById('route-path');
  const routeDot = document.getElementById('route-dot');
  const stage = document.getElementById('stage');
  const player = document.getElementById('player');
  const sourcePlayer = document.getElementById('source-player');
  const progress = document.getElementById('morph-position');
  const mq = matchMedia('(prefers-reduced-motion: reduce)');
  const names = { text: 'Words', image: 'Image', audio: 'Sound', json: 'Analysis' };
  const TAU = Math.PI * 2, count = 144;
  const model = { blend: 0 };
  const mediaSources = new WeakMap();
  const luminance = { source: null, result: null };
  const textLines = { source: [' '], result: [' '] };
  let from = 'text', to = 'audio', enabled = !mq.matches, busy = false, completed = false;
  let sourceSeed = 1, resultSeed = 2, sourceText = '', resultText = '', dataValues = [];
  let fieldAnimation, routeAnimation, revealAnimation, frame = null, lastFrame = 0;
  let audioContext, analyser, wave, frequency, level = 0;
  // The system preference supplies only the default; an explicit choice wins.
  try {
    const saved = localStorage.getItem('xformed-motion');
    if (saved === 'on' || saved === 'off') enabled = saved === 'on';
  } catch {}

  function hash(value) {
    let h = 2166136261;
    for (const c of value) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
    return h >>> 0;
  }
  function active() { return enabled && !document.hidden; }
  function stop(animation) { animation?.revert(); }
  function resize() {
    const r = canvas.getBoundingClientRect(), d = Math.min(devicePixelRatio || 1, 2);
    canvas.width = Math.round(r.width * d);
    canvas.height = Math.round(r.height * d);
    paint(0);
  }
  function updateSource() {
    sourceText = document.getElementById('text').value;
    textLines.source = sourceText.slice(0, 1200).match(/.{1,48}(?:\s|$)/g) || [' '];
    sourceSeed = hash(from === 'text' ? sourceText : document.getElementById('file-info').textContent);
    paint(0);
  }
  function sampleImage(image, side) {
    if (!image.complete || !image.naturalWidth) return;
    try {
      const sample = document.createElement('canvas');
      sample.width = sample.height = 12;
      const sampleContext = sample.getContext('2d', { willReadFrequently: true });
      sampleContext.drawImage(image, 0, 0, 12, 12);
      const rgba = sampleContext.getImageData(0, 0, 12, 12).data;
      luminance[side] = Array.from({ length: count }, (_, i) =>
        (rgba[i * 4] * .2126 + rgba[i * 4 + 1] * .7152 + rgba[i * 4 + 2] * .0722) / 255);
      paint(0);
    } catch { luminance[side] = null; }
  }
  document.getElementById('preview').addEventListener('load', event => sampleImage(event.target, 'source'));
  document.getElementById('generated-image').addEventListener('load', event => sampleImage(event.target, 'result'));
  document.getElementById('text').addEventListener('input', updateSource);
  document.getElementById('file').addEventListener('change', updateSource);

  function animateRoute() {
    stop(routeAnimation);
    routeDot.style.transform = '';
    routeDot.style.opacity = active() ? '1' : '0';
    if (active() && window.anime) routeAnimation = anime.animate(routeDot, {
      ...anime.svg.createMotionPath(routePath), duration: busy ? 1000 : 4000, ease: 'linear', loop: true
    });
  }
  function animateField() {
    stop(fieldAnimation);
    if (active() && window.anime) {
      fieldAnimation = anime.animate(model, {
        blend: [0, 1], duration: busy ? 1800 : 4200, delay: busy ? 0 : 700,
        loopDelay: 700, alternate: true, loop: true, ease: 'inOutSine'
      });
    }
  }
  function layoutRoute() {
    // The three creative modalities form a triangle; analysis sits at its center.
    // Keep SVG units in CSS pixels so labels remain readable as the panel resizes.
    const width = Math.max(160, routeMap.getBoundingClientRect().width);
    routeMap.setAttribute('viewBox', `0 0 ${width} 218`);
    const center = width / 2, spread = Math.min(88, (width - 60) / 2);
    const sockets = { text: [center, 36], image: [center - spread, 172], audio: [center + spread, 172], json: [center, 116] };
    routeMap.querySelectorAll('[data-route-port]').forEach(port => {
      port.setAttribute('transform', `translate(${sockets[port.dataset.routePort].join(' ')})`);
    });
    const [top, left, right, core] = ['text', 'image', 'audio', 'json'].map(kind => sockets[kind].join(' '));
    routeMap.querySelector('.route-grid').setAttribute('d', `M${top}L${left}L${right}Z`);
    routeMap.querySelector('.route-spokes').setAttribute('d', `M${top}L${core}M${left}L${core}M${right}L${core}`);
    const a = sockets[from], b = sockets[to], dx = b[0] - a[0], dy = b[1] - a[1];
    const length = Math.hypot(dx, dy), ux = dx / length, uy = dy / length;
    const start = [a[0] + ux * (from === 'json' ? 9 : 12), a[1] + uy * (from === 'json' ? 9 : 12)];
    const end = [b[0] - ux * (to === 'json' ? 11 : 14), b[1] - uy * (to === 'json' ? 11 : 14)];
    const midpoint = [(start[0] + end[0]) / 2, (start[1] + end[1]) / 2];
    const outward = (-uy * (midpoint[0] - center) + ux * (midpoint[1] - 126)) >= 0 ? 1 : -1;
    const bend = from === 'json' || to === 'json' ? 0 : 18 * outward;
    routePath.setAttribute('d', `M${start.join(' ')}Q${midpoint[0] - uy * bend} ${midpoint[1] + ux * bend} ${end.join(' ')}`);
    animateRoute();
  }
  function setRoute(source, destination) {
    from = source; to = destination; completed = false;
    canvas.dataset.complete = 'false';
    resultText = ''; dataValues = []; luminance.result = null;
    textLines.result = [' '];
    luminance.source = null;
    sampleImage(document.getElementById('preview'), 'source');
    updateSource(); resultSeed = hash(source + destination + sourceSeed);
    canvas.dataset.from = from; canvas.dataset.to = to;
    canvas.setAttribute('aria-label', `${names[from]} transforming into ${names[to]}`);
    document.getElementById('morph-from').textContent = names[from];
    document.getElementById('morph-to').textContent = names[to];
    const destinationName = { text: 'Writing', image: 'Artwork', audio: 'Music', json: 'Analysis' }[to];
    document.getElementById('route-code').textContent = `${names[from]} → ${destinationName}`;
    routeMap.querySelectorAll('[data-route-port]').forEach(port => {
      port.classList.toggle('is-source', port.dataset.routePort === from);
      port.classList.toggle('is-target', port.dataset.routePort === to);
    });
    layoutRoute(); animateField(); paint(0);
  }
  function result(response, context) {
    const writing = response.artifacts.find(a => a.type === 'Text');
    const info = response.artifacts.find(a => a.type === 'CreativeInfo');
    resultText = writing?.text || '';
    textLines.result = resultText.slice(0, 1200).match(/.{1,48}(?:\s|$)/g) || [' '];
    resultSeed = hash(info?.seed || JSON.stringify(context.options) + context.sourceLabel);
    const analysis = response.artifacts.find(a => a.type === 'Json');
    if (analysis) {
      const numbers = [];
      const collect = object => {
        for (const value of Object.values(object || {})) {
          if (numbers.length >= count) break;
          if (typeof value === 'number' && Number.isFinite(value)) numbers.push(Math.log1p(Math.abs(value)));
          else if (value && typeof value === 'object') collect(value);
        }
      };
      collect(analysis.data);
      const max = Math.max(1, ...numbers);
      dataValues = numbers.map(value => value / max);
    }
    completed = true;
    canvas.dataset.complete = 'true';
    updateSource();
  }

  async function connect(element) {
    try {
      if (!audioContext) {
        audioContext = new (window.AudioContext || window.webkitAudioContext)();
        analyser = audioContext.createAnalyser(); analyser.fftSize = 512;
        wave = new Uint8Array(analyser.fftSize); frequency = new Uint8Array(analyser.frequencyBinCount);
        analyser.connect(audioContext.destination);
      }
      if (!mediaSources.has(element)) {
        const source = audioContext.createMediaElementSource(element);
        source.connect(analyser); mediaSources.set(element, source);
      }
      if (audioContext.state === 'suspended') await audioContext.resume();
    } catch {}
  }
  player.addEventListener('play', () => connect(player));
  sourcePlayer.addEventListener('play', () => connect(sourcePlayer));

  function point(kind, i, time, side) {
    const u = i / (count - 1), seed = side === 'source' ? sourceSeed : resultSeed;
    if (kind === 'audio') {
      const playing = !player.paused || !sourcePlayer.paused;
      const signal = wave && playing ? (wave[Math.floor(u * (wave.length - 1))] - 128) / 128
        : Math.sin(u * TAU * (2 + seed % 4) + time * .8) * .3
          + Math.sin(u * TAU * (7 + seed % 11) - time * .4) * .12;
      return [u * 1.9 - .95, signal * 1.4];
    }
    if (kind === 'image') {
      const col = i % 12, row = Math.floor(i / 12);
      const light = luminance[side]?.[i] ?? (.5 + Math.sin(col * .6 + row * .5 + seed % 31) * .25);
      return [(col / 11 - .5) * 1.75 + Math.sin(row * .4 + time * .25) * .025,
        (row / 11 - .5) * 1.4 + (light - .5) * .38];
    }
    if (kind === 'json') {
      const bar = Math.floor(i / 12), step = i % 12;
      const value = dataValues.length ? dataValues[bar % dataValues.length] : .25 + ((seed >>> (bar % 24)) % 61) / 100;
      return [(bar / 11 - .5) * 1.8, .7 - step / 11 * (value * 1.2 + .2)];
    }
    const lines = textLines[side];
    const row = Math.floor(i / 24), col = i % 24;
    const line = lines[row % lines.length] || ' ';
    const width = Math.max(.35, Math.min(1, line.length / 45));
    const char = line.charCodeAt(col % line.length) || seed;
    return [-.88 + col / 23 * 1.75 * width, (row / 5 - .5) * 1.35 + Math.sin(char + col * .6) * .025];
  }
  function connections(points, kind, alpha, color) {
    if (alpha < .01) return;
    ctx.globalAlpha = alpha; ctx.strokeStyle = color; ctx.lineWidth = Math.max(.8, canvas.width / 1100);
    ctx.beginPath();
    const rowWidth = kind === 'audio' ? count : kind === 'text' ? 24 : 12;
    for (let i = 1; i < count; i++) {
      if (i % rowWidth !== 0) { ctx.moveTo(...points[i - 1]); ctx.lineTo(...points[i]); }
      if (kind === 'image' && i >= 12) { ctx.moveTo(...points[i - 12]); ctx.lineTo(...points[i]); }
    }
    ctx.stroke();
  }
  function paint(time) {
    const w = canvas.width, h = canvas.height;
    if (!w || !h) return;
    ctx.clearRect(0, 0, w, h);
    const playing = !player.paused || !sourcePlayer.paused;
    if (analyser && playing && active()) {
      analyser.getByteTimeDomainData(wave); analyser.getByteFrequencyData(frequency);
      level = frequency.slice(0, 40).reduce((a, b) => a + b, 0) / 40 / 255;
    } else level *= .85;
    const blend = active() ? model.blend : .5;
    progress.style.transform = `scaleX(${blend})`;
    canvas.dataset.phase = blend < .1 ? 'source' : blend > .9 ? 'result' : 'transition';
    const positions = Array.from({ length: count }, (_, i) => {
      const a = point(from, i, time, 'source'), b = point(to, i, time, 'result');
      return [(a[0] * (1 - blend) + b[0] * blend) * w * .47 + w * .5,
        (a[1] * (1 - blend) + b[1] * blend) * h * .46 + h * .5];
    });
    connections(positions, from, .38 * (1 - blend), '#385642');
    connections(positions, to, .65 * blend, '#264ae3');
    ctx.globalAlpha = 1;
    for (let i = 0; i < count; i++) {
      ctx.fillStyle = i % 12 === 0 ? '#264ae3' : '#395449';
      ctx.beginPath();
      ctx.arc(...positions[i], Math.max(1.1, w / 550) * (i % 12 === 0 ? 1.6 : .8) + level, 0, TAU);
      ctx.fill();
    }
  }
  function loop(now) {
    if (!active()) { frame = null; return; }
    frame = requestAnimationFrame(loop);
    if (now - lastFrame < 40) return;
    lastFrame = now; paint(now / 1000);
  }
  function preference() {
    const motion = enabled;
    document.body.classList.toggle('motion-on', motion);
    document.body.classList.toggle('motion-off', !motion);
    const toggle = document.getElementById('motion');
    toggle.textContent = motion ? 'Motion on' : 'Motion off';
    toggle.setAttribute('aria-pressed', String(motion));
    stop(fieldAnimation); stop(revealAnimation);
    stage.style.opacity = '1'; stage.style.transform = '';
    stage.style.removeProperty('--tilt-x'); stage.style.removeProperty('--tilt-y');
    if (frame !== null) cancelAnimationFrame(frame);
    frame = null; animateRoute(); animateField(); paint(0);
    if (active()) frame = requestAnimationFrame(loop);
  }
  document.getElementById('motion').onclick = () => {
    enabled = !enabled;
    try { localStorage.setItem('xformed-motion', enabled ? 'on' : 'off'); } catch {}
    preference();
  };
  document.addEventListener('visibilitychange', preference);
  stage.addEventListener('pointermove', event => {
    if (!active() || event.pointerType === 'touch') return;
    const r = stage.getBoundingClientRect();
    stage.style.setProperty('--tilt-x', `${((event.clientX - r.left) / r.width - .5) * 2.4}deg`);
    stage.style.setProperty('--tilt-y', `${-((event.clientY - r.top) / r.height - .5) * 2.4}deg`);
  });
  stage.addEventListener('pointerleave', () => {
    stage.style.setProperty('--tilt-x', '0deg'); stage.style.setProperty('--tilt-y', '0deg');
  });
  new ResizeObserver(resize).observe(canvas.parentElement);
  new ResizeObserver(layoutRoute).observe(routeMap);
  resize(); preference();
  window.XformedEffects = {
    route: setRoute, source: updateSource, result, mood() {},
    busy(value) {
      busy = value;
      if (value) { completed = false; canvas.dataset.complete = 'false'; }
      animateRoute(); animateField();
    },
    reveal() {
      completed = true; canvas.dataset.complete = 'true';
      stop(revealAnimation);
      if (active() && window.anime) revealAnimation = anime.createTimeline()
        .add('#result-title', { opacity: [0, 1], translateY: [7, 0], duration: 320, ease: 'outCubic' }, 0)
        .add('#stage', { opacity: [.12, 1], translateY: [12, 0], duration: 570, ease: 'outExpo' }, 50);
      paint(0);
    }
  };
})();
