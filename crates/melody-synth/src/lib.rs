//! Melody Synth – "pe bune": layering, polyphony, swing, humanize, percussion.
//! Copy–paste over your file if you want this implementation wholesale.
//!
//! Requires in Cargo.toml:
//! [dependencies]
//! anyhow = "1"
//! hound = "3"
//!
//! Assumptions about melody_core::MonophonicMidi:
//! - You can iterate its notes (IntoIterator or a .iter() that yields items with
//!   fields: pitch: u8, t_on: f32 (sec), t_off: f32 (sec), velocity: u8).
//!   If your API differs, adjust in `collect_events(...)` (marked in file).
//!
//! Non-breaking API: we keep `render_wav_bytes(...)` and add `StyleParams` + `render_wav_bytes_styled(...)`.

use anyhow::{anyhow, Result};
use hound::{SampleFormat, WavSpec, WavWriter};
use melody_core::{MonophonicMidi, ScaleKind};

use std::f32::consts::PI;
use std::io::Cursor;

/* =========================
   Public types & API
   ========================= */

#[derive(Clone, Copy, Debug)]
pub enum Osc {
    Sine,
    Saw,
    Square,
}

/// High-level style for rendering.
#[derive(Clone, Debug)]
pub struct StyleParams {
    /// Timbre layers; first is primary. Ex: [Saw, Sine, Square]
    pub layering: Vec<Osc>,
    /// Delay on even notes (0..0.35). 0 = no swing.
    pub swing: f32,
    /// Small timing/velocity jitter (0..0.4).
    pub humanize: f32,
    /// 1 = mono line; 2 = dyad; 3 = triad (root+third+fifth).
    pub polyphony: usize,
    /// Add a simple drum channel aligned to tempo.
    pub percussion: bool,
    /// Scale kind for choosing the third (major/minor) when polyphony > 1
    pub scale: ScaleKind,
}

impl Default for StyleParams {
    fn default() -> Self {
        Self {
            layering: vec![Osc::Saw, Osc::Sine],
            swing: 0.0,
            humanize: 0.1,
            polyphony: 1,
            percussion: false,
            scale: ScaleKind::Major,
        }
    }
}

/// Legacy API preserved: renders with a single primary oscillator.
/// Internally we call the styled path with a simple style.
pub fn render_wav_bytes(midi: &MonophonicMidi, sr: u32, primary: Osc) -> Result<Vec<u8>> {
    let style = StyleParams {
        layering: vec![primary],
        swing: 0.0,
        humanize: 0.0,
        polyphony: 1,
        percussion: false,
        scale: ScaleKind::Major,
    };
    render_wav_bytes_styled(midi, sr, &style)
}

/// New API: full "pe bune" rendering with layering/polyphony/swing/humanize/percussion.
pub fn render_wav_bytes_styled(midi: &MonophonicMidi, sr: u32, style: &StyleParams) -> Result<Vec<u8>> {
    if style.layering.is_empty() {
        return Err(anyhow!("StyleParams.layering must contain at least one oscillator"));
    }

    // 1) Collect note events from MIDI
    let mut events = collect_events(midi)?;

    // 2) Estimate tempo in BPM if not available elsewhere (used for drums & swing scale)
    let bpm = estimate_bpm(&events).unwrap_or(120.0);

    // 3) Apply swing & humanize
    apply_swing_and_humanize(&mut events, style.swing, style.humanize, bpm);

    // 4) Expand polyphony (triads/dyads) by cloning events and transposing by scale intervals
    if style.polyphony > 1 {
        expand_polyphony(&mut events, style.polyphony, style.scale);
    }

    // 5) Render note layers into a mono buffer
    let total_len = calc_total_len(&events);
    let total_samples = (total_len * sr as f32).ceil() as usize + (sr as usize / 2); // tail 0.5s
    let mut out = vec![0.0f32; total_samples];

    // Layer detune/gain recipe (depends on chosen layering)
    let layer_specs = layering_specs(&style.layering);

    // variație: rotim layerele pe parcurs în „secțiuni” ~ 8 sec
    let section_len = 8.0_f32;
    for (idx, ev) in events.iter().enumerate() {
        let sec_idx = (ev.t_on / section_len).floor() as usize;
        // rotim ordinea layerelor în funcție de secțiune + index
        let mut rotated = layer_specs.clone();
        if !rotated.is_empty() {
            let r = (sec_idx + idx / 32) % rotated.len();
            rotated.rotate_left(r);
        }

        for spec in &rotated {
            let f0 = midi_pitch_to_hz(ev.pitch) * cents_to_ratio(spec.detune_cents);
            // mică variație de gain în timp (pulsare subtilă)
            let g_time = 0.9 + 0.1 * ((ev.t_on * 1.3).sin()).abs();
            render_note(
                &mut out, sr, f0, ev.t_on, ev.t_off,
                (ev.velocity as f32 / 127.0) * spec.gain * g_time as f32,
                spec.osc
            );
        }
    }

    // 6) Drums channel (optional)
    if style.percussion {
        render_drums(&mut out, sr, bpm);
    }

    // 7) Normalize softly to avoid clipping
    normalize_soft(&mut out, 0.99);

    // 8) Encode to WAV 16-bit PCM in-memory
    write_wav_i16(&out, sr)
}

/* =========================
   Internal: MIDI → events
   ========================= */

#[derive(Clone, Copy, Debug)]
struct NoteEv {
    pitch: u8,
    t_on: f32,
    t_off: f32,
    velocity: u8,
}

/// Try to iterate MIDI notes and collect them as NoteEv.
/// ADAPTEAZĂ AICI dacă structura ta diferă.
/// Necesită ca MonophonicMidi să fie iterabil sau să aibă .iter().
fn collect_events(midi: &MonophonicMidi) -> Result<Vec<NoteEv>> {
    let mut evs = Vec::new();

    // Dacă MonophonicMidi are .notes: Vec<Note>
    for n in midi.notes.iter() {
        let pitch: u8 = n.pitch;
        // start/end pot fi f32 sau f64 în implementarea ta — convertim la f32
        let t_on: f32  = n.start as f32;
        let t_off: f32 = n.end as f32;
        let velocity: u8 = n.velocity;

        if t_off > t_on {
            evs.push(NoteEv { pitch, t_on, t_off, velocity });
        }
    }

    if evs.is_empty() {
        return Err(anyhow!("No notes in MonophonicMidi (collect_events)"));
    }
    Ok(evs)
}

/// Estimate BPM from median note duration (very rough).
fn estimate_bpm(evs: &[NoteEv]) -> Option<f32> {
    if evs.is_empty() { return None; }
    let mut ds: Vec<f32> = evs.iter().map(|e| e.t_off - e.t_on).collect();
    ds.sort_by(|a,b| a.partial_cmp(b).unwrap());
    let med = ds[ds.len()/2];
    if med <= 0.0 { return None; }
    // Assume the common case where durations ≈ 0.5 beat (eighth notes)
    let beats = med / 0.5;
    let bpm = 60.0 * beats.max(0.1);
    Some(bpm.clamp(50.0, 200.0))
}

fn calc_total_len(evs: &[NoteEv]) -> f32 {
    evs.iter().fold(0.0, |mx, e| mx.max(e.t_off))
}

/* =========================
   Swing & Humanize
   ========================= */

fn apply_swing_and_humanize(evs: &mut [NoteEv], swing: f32, human: f32, bpm: f32) {
    if evs.is_empty() { return; }
    let swing = swing.clamp(0.0, 0.35);
    let human = human.clamp(0.0, 0.4);

    // Compute nominal eighth duration from BPM
    let eighth = 60.0 / bpm / 2.0;

    for (i, e) in evs.iter_mut().enumerate() {
        // Swing: delay even-indexed notes by a fraction of eighth
        if (i & 1) == 1 && swing > 0.0 {
            let shift = swing * 0.5 * eighth;
            e.t_on += shift;
            e.t_off += shift;
        }
        if human > 0.0 {
            // Timing jitter ±2% of note length scaled by human
            let dur = (e.t_off - e.t_on).max(1e-4);
            let jt = (rand_hash(i as u64) * 2.0 - 1.0) * 0.02 * human * dur;
            e.t_on = (e.t_on + jt).max(0.0);
            e.t_off = (e.t_off + jt).max(e.t_on + 1e-4);

            // Velocity jitter ±12% scaled by human
            let jv = 1.0 + (rand_hash((i as u64) ^ 0x9E3779B97F4A7C15) * 2.0 - 1.0) * 0.12 * human;
            let vv = (e.velocity as f32 * jv).clamp(1.0, 127.0);
            e.velocity = vv as u8;
        }
    }
}

/* =========================
   Polyphony expansion
   ========================= */

fn expand_polyphony(evs: &mut Vec<NoteEv>, voices: usize, scale: ScaleKind) {
    let voices = voices.min(3).max(1);
    if voices == 1 { return; }

    // Copy original events
    let base = evs.clone();
    let (third_semi, fifth_semi) = match scale {
        ScaleKind::Major => (4i32, 7i32),
        ScaleKind::Minor => (3i32, 7i32),
    };

    if voices >= 2 {
        for e in &base {
            let p = (e.pitch as i32 + third_semi).clamp(0, 127) as u8;
            evs.push(NoteEv { pitch: p, ..*e });
        }
    }
    if voices >= 3 {
        for e in &base {
            let p = (e.pitch as i32 + fifth_semi).clamp(0, 127) as u8;
            evs.push(NoteEv { pitch: p, ..*e });
        }
    }

    // Sort by t_on to keep timeline reasonable
    evs.sort_by(|a,b| a.t_on.partial_cmp(&b.t_on).unwrap());
}

/* =========================
   Layering (detune & gain)
   ========================= */

#[derive(Clone, Copy)]
struct LayerSpec { osc: Osc, detune_cents: f32, gain: f32 }

fn layering_specs(list: &[Osc]) -> Vec<LayerSpec> {
    if list.is_empty() {
        return vec![LayerSpec { osc: Osc::Saw, detune_cents: 0.0, gain: 1.0 }];
    }
    let mut specs = Vec::new();
    for (i, &osc) in list.iter().enumerate() {
        let (det, g) = match (osc, i) {
            (Osc::Saw, 0)    => (  0.0, 0.70),
            (Osc::Saw, 1)    => (  7.0, 0.20),
            (Osc::Saw, _)    => ( -4.0, 0.10),
            (Osc::Square, 0) => (  0.0, 0.70),
            (Osc::Square, 1) => (  5.0, 0.20),
            (Osc::Square, _) => ( -5.0, 0.10),
            (Osc::Sine, 0)   => (  0.0, 0.80),
            (Osc::Sine, 1)   => ( 12.0, 0.15), // octave up hint
            (Osc::Sine, _)   => (  4.0, 0.05),
        };
        specs.push(LayerSpec { osc, detune_cents: det, gain: g });
    }
    specs
}

/* =========================
   Rendering: oscillators & notes
   ========================= */

fn midi_pitch_to_hz(p: u8) -> f32 {
    440.0 * 2f32.powf((p as f32 - 69.0) / 12.0)
}

fn cents_to_ratio(cents: f32) -> f32 {
    2f32.powf(cents / 1200.0)
}

fn osc_sample(osc: Osc, phase: f32) -> f32 {
    match osc {
        Osc::Sine => (2.0 * PI * phase).sin(),
        Osc::Saw => 2.0 * (phase.fract()) - 1.0,
        Osc::Square => if (phase.fract()) < 0.5 { 1.0 } else { -1.0 },
    }
}

// very small click-free envelope (attack/decay only)
fn ad_env(rel: f32) -> f32 {
    // simple exponential-ish (0..1)
    // rel in [0,1]; fast attack, gentle decay
    let a = if rel < 0.02 { rel / 0.02 } else { 1.0 };
    let d = 1.0 - ((rel).powf(1.5)).min(1.0);
    a * d
}

fn render_note(out: &mut [f32], sr: u32, f0: f32, t_on: f32, t_off: f32, gain: f32, osc: Osc) {
    if t_off <= t_on { return; }
    let sr_f = sr as f32;
    let start = (t_on * sr_f).max(0.0) as usize;
    let end = ((t_off * sr_f) as usize).min(out.len());
    if end <= start { return; }

    let mut phase = 0.0f32;
    let inc = f0 / sr_f;

    let dur = (end - start).max(1) as f32;
    for i in start..end {
        let rel = (i - start) as f32 / dur;
        let env = ad_env(rel);
        let s = osc_sample(osc, phase) * env * gain;
        out[i] += s;
        phase += inc;
        if phase >= 1.0 { phase -= 1.0; }
    }
}

/* =========================
   Drums: kick/snare/hat
   ========================= */

fn render_drums(out: &mut [f32], sr: u32, bpm: f32) {
    let sr_f = sr as f32;
    let spb = 60.0 / bpm; // seconds per beat
    let eighth = spb / 2.0;

    // We'll lay hats on every eighth, kick on beats 1 & 3, snare on 2 & 4 (4/4)
    let total_secs = out.len() as f32 / sr_f;
    let mut t = 0.0;
    let mut idx = 0usize;
    while t < total_secs {
        let beat_num = (t / spb).floor() as i32;
        let in_bar = beat_num % 4;
        let is_beat = (t % spb) < 1e-6;

        // Kick on 1 & 3
        if is_beat && (in_bar == 0 || in_bar == 2) {
            render_kick(out, sr, t, 0.18, 75.0, 45.0);
        }
        // Snare on 2 & 4
        if is_beat && (in_bar == 1 || in_bar == 3) {
            render_snare(out, sr, t + 0.005, 0.14, 0.6);
        }
        // Hats every eighth
        render_hat(out, sr, t, 0.05, 0.25);

        idx += 1;
        t = idx as f32 * eighth;
    }
}

fn render_kick(out: &mut [f32], sr: u32, t_on: f32, dur: f32, start_hz: f32, end_hz: f32) {
    let start = (t_on * sr as f32) as usize;
    let end = ((t_on + dur) * sr as f32) as usize;
    if end <= start || end > out.len() { return; }
    let mut phase = 0.0f32;
    for i in start..end {
        let rel = (i - start) as f32 / ((end - start) as f32);
        let freq = start_hz + (end_hz - start_hz) * rel;
        let inc = freq / sr as f32;
        let env = (1.0 - rel).powf(4.0); // sharp decay
        let s = (2.0 * PI * phase).sin() * env * 0.9;
        out[i] += s;
        phase = (phase + inc) % 1.0;
    }
}

fn render_snare(out: &mut [f32], sr: u32, t_on: f32, dur: f32, tone: f32) {
    // noise + short tone
    let start = (t_on * sr as f32) as usize;
    let end = ((t_on + dur) * sr as f32) as usize;
    if end <= start || end > out.len() { return; }
    let mut phase = 0.0f32;
    let inc = 220.0 / sr as f32;
    for i in start..end {
        let rel = (i - start) as f32 / ((end - start) as f32);
        let env = (1.0 - rel).powf(3.0);
        // tone
        let t = (2.0 * PI * phase).sin() * tone * env * 0.4;
        phase = (phase + inc) % 1.0;
        // noise
        let n = (rand_hash(i as u64) * 2.0 - 1.0) * env * 0.6;
        out[i] += t + n;
    }
}

fn render_hat(out: &mut [f32], sr: u32, t_on: f32, dur: f32, gain: f32) {
    let start = (t_on * sr as f32) as usize;
    let end = ((t_on + dur) * sr as f32) as usize;
    if end <= start || end > out.len() { return; }
    // bright noise with HP-ish response
    for i in start..end {
        let rel = (i - start) as f32 / ((end - start) as f32);
        let env = (1.0 - rel).powf(4.0);
        let n = rand_hash((i * 13) as u64) * 2.0 - 1.0;
        // crude "HPF": subtract a smoothed version
        let bright = n - 0.5 * (rand_hash((i * 11) as u64) * 2.0 - 1.0);
        out[i] += bright * env * gain;
    }
}

/* =========================
   Utils: normalize & WAV writer
   ========================= */

fn normalize_soft(buf: &mut [f32], target_peak: f32) {
    let mut peak = 0.0f32;
    for &x in buf.iter() { peak = peak.max(x.abs()); }
    if peak > target_peak && peak > 1e-9 {
        let k = target_peak / peak;
        for x in buf.iter_mut() { *x *= k; }
    }
}

fn write_wav_i16(buf: &[f32], sr: u32) -> Result<Vec<u8>> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let capacity = buf.len() * 2 + 64;
    let mut cursor = Cursor::new(Vec::with_capacity(capacity));
    {
        let mut writer = WavWriter::new(&mut cursor, spec)?;
        for &s in buf {
            let v = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            writer.write_sample(v)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

/* =========================
   Tiny PRNG (deterministic but simple)
   ========================= */

fn rand_hash(mut x: u64) -> f32 {
    // xorshift-ish
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    ((x.wrapping_mul(0x2545F4914F6CDD1D) >> 33) as f32) / (u32::MAX as f32)
}
