// Small original source examples, generated locally. No download or uploaded files.
(() => {
  async function image() {
    const canvas = document.createElement('canvas'); canvas.width = 384; canvas.height = 256;
    const c = canvas.getContext('2d');
    const base = c.createLinearGradient(0, 0, 384, 256);
    base.addColorStop(0, '#173a48'); base.addColorStop(.5, '#5b7e65'); base.addColorStop(1, '#e4bf82');
    c.fillStyle = base; c.fillRect(0, 0, 384, 256);
    for (let i = 0; i < 8; i++) {
      c.beginPath();
      for (let x = 0; x <= 384; x += 3) {
        const y = 60 + i * 23 + Math.sin(x / (38 + i * 4) + i * .7) * (12 + i * 3);
        if (!x) c.moveTo(x, y); else c.lineTo(x, y);
      }
      c.lineTo(384, 256); c.lineTo(0, 256); c.closePath();
      c.fillStyle = `rgba(11,45,53,${.1 + i * .035})`; c.fill();
    }
    c.fillStyle = '#f3dca5'; c.beginPath(); c.arc(286, 64, 24, 0, Math.PI * 2); c.fill();
    const blob = await new Promise(resolve => canvas.toBlob(resolve, 'image/png'));
    return new File([blob], 'example-landscape.png', { type: 'image/png' });
  }
  function audio() {
    const rate = 22050, seconds = 6, samples = rate * seconds;
    const buffer = new ArrayBuffer(44 + samples * 2), view = new DataView(buffer);
    const text = (offset, value) => { for (let i = 0; i < value.length; i++) view.setUint8(offset + i, value.charCodeAt(i)); };
    text(0, 'RIFF'); view.setUint32(4, 36 + samples * 2, true); text(8, 'WAVE');
    text(12, 'fmt '); view.setUint32(16, 16, true); view.setUint16(20, 1, true); view.setUint16(22, 1, true);
    view.setUint32(24, rate, true); view.setUint32(28, rate * 2, true); view.setUint16(32, 2, true); view.setUint16(34, 16, true);
    text(36, 'data'); view.setUint32(40, samples * 2, true);
    const notes = [261.63, 329.63, 392, 493.88, 440, 392, 329.63, 293.66];
    for (let i = 0; i < samples; i++) {
      const t = i / rate, onset = t % .375, pitch = notes[Math.floor(t / .375) % notes.length];
      const envelope = Math.min(1, onset * 80) * Math.exp(-onset * 9) * Math.min(1, (seconds - t) * 3);
      const sample = (Math.sin(t * pitch * Math.PI * 2) + Math.sin(t * pitch * Math.PI * 4) * .2) * envelope * .36;
      view.setInt16(44 + i * 2, Math.round(sample * 32767), true);
    }
    return new File([buffer], 'example-phrase.wav', { type: 'audio/wav' });
  }
  window.XformedExamples = { image, audio };
})();
