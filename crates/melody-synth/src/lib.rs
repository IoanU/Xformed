
use anyhow::Result;
use hound::{WavSpec, WavWriter, SampleFormat};
use melody_core::MonophonicMidi;
use std::io::Cursor;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum Osc { Sine, Square, Saw }

pub fn render_wav_bytes(m: &MonophonicMidi, sr: u32, osc: Osc) -> Result<Vec<u8>> {
    // Use a Cursor<Vec<u8>> because WavWriter requires Write + Seek
    let mut cursor = Cursor::new(Vec::<u8>::new());
    {
        let spec = WavSpec{ channels: 1, sample_rate: sr, bits_per_sample: 16, sample_format: SampleFormat::Int };
        let mut w = WavWriter::new(&mut cursor, spec)?;

        for n in &m.notes {
            let freq = melody_core::midi_to_hz(n.pitch as f32);
            let dur_s = (n.end - n.start).max(0.01);
            let samples = (dur_s * sr as f32) as usize;
            for i in 0..samples {
                let t = i as f32 / sr as f32;
                let s = osc_sample(osc.clone(), freq, t) * env(i, samples) * (n.velocity as f32 / 127.0);
                w.write_sample((s * i16::MAX as f32) as i16)?;
            }
        }
        w.finalize()?;
    }
    Ok(cursor.into_inner())
}

fn osc_sample(osc: Osc, f: f32, t: f32) -> f32 {
    let x = 2.0*std::f32::consts::PI*f*t;
    match osc {
        Osc::Sine => x.sin(),
        Osc::Square => if x.sin()>=0.0 {1.0}else{-1.0},
        Osc::Saw => 2.0*((t*f) - (t*f).floor()) - 1.0
    }
}
fn env(i: usize, n: usize) -> f32 {
    let a = (0.02*n as f32).max(1.0) as usize;
    let r = (0.05*n as f32).max(1.0) as usize;
    let amp_a = (i as f32 / a as f32).min(1.0);
    let amp_r = if i>n.saturating_sub(r) { 1.0 - ((i - (n-r)) as f32) / r as f32 } else { 1.0 };
    amp_a.min(amp_r)
}
