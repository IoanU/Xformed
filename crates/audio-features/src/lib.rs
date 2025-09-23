
use anyhow::Result;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::{get_codecs, get_probe};

/// Decode common audio formats (wav/mp3/flac/aac in mp4) to mono f32 samples and sample rate.
pub fn decode_to_mono(bytes: &[u8]) -> Result<(Vec<f32>, u32)> {
    let cursor = std::io::Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    // Provide no hints.
    let hint = Hint::new();

    // Use the default probe to guess format.
    let probed = get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())?;
    let mut format = probed.format;

    // Get the default audio track; clone codec params to build the decoder.
    let track = format.default_track().ok_or_else(|| anyhow::anyhow!("no default track"))?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();

    // Create a decoder.
    let mut decoder = get_codecs().make(&codec_params, &DecoderOptions::default())?;

    let mut out = Vec::<f32>::new();
    let mut sr = 44_100u32;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(_)) => break,
            Err(Error::ResetRequired) => break, // stream reset not handled in MVP
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                sr = audio_buf.spec().rate;
                let chans = audio_buf.spec().channels.count();

                // Copy samples into a contiguous f32 buffer.
                let mut sample_buf = SampleBuffer::<f32>::new(audio_buf.capacity() as u64, *audio_buf.spec());
                sample_buf.copy_interleaved_ref(audio_buf);

                let samples = sample_buf.samples();
                if chans == 1 {
                    out.extend_from_slice(samples);
                } else {
                    // Mixdown to mono.
                    for frame in samples.chunks_exact(chans) {
                        let mut acc = 0.0f32;
                        for &s in frame { acc += s; }
                        out.push(acc / chans as f32);
                    }
                }
            }
            Err(Error::DecodeError(_)) => {
                // Recoverable decode error, skip packet.
                continue;
            }
            Err(e) => return Err(anyhow::anyhow!(e)),
        }
    }

    Ok((out, sr))
}

/// Compute frame indices (start) for a signal given window and hop sizes.
fn frames_indices(len: usize, win: usize, hop: usize) -> impl Iterator<Item=usize> {
    (0..).map(move |i| i*hop).take_while(move |&s| s + win <= len)
}

/// Hann window
fn hann(n: usize) -> Vec<f32> {
    let mut w = vec![0f32; n];
    let c = std::f32::consts::PI * 2.0 / (n as f32);
    for i in 0..n { w[i] = 0.5 - 0.5 * (c * (i as f32)).cos(); }
    w
}

/// Spectral flux (for onset detection) using rustfft.
pub fn spectral_flux(samples: &[f32], _sr: u32, win: usize, hop: usize) -> Vec<f32> {
    use rustfft::{FftPlanner, num_complex::Complex32};
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(win);
    let window = hann(win);
    let mut prev_mag: Vec<f32> = vec![0.0; win];
    let mut flux: Vec<f32> = Vec::new();

    let mut buf: Vec<Complex32> = vec![Complex32::new(0.0, 0.0); win];
    for start in frames_indices(samples.len(), win, hop) {
        for i in 0..win {
            let s = samples[start + i] * window[i];
            buf[i] = Complex32::new(s, 0.0);
        }
        fft.process(&mut buf);
        // magnitude spectrum
        let mut sum_pos = 0.0f32;
        for i in 0..(win/2) {
            let m = (buf[i].re * buf[i].re + buf[i].im * buf[i].im).sqrt();
            let d = (m - prev_mag[i]).max(0.0);
            sum_pos += d;
            prev_mag[i] = m;
        }
        flux.push(sum_pos);
    }
    // normalize
    let maxv = flux.iter().cloned().fold(0.0f32, f32::max).max(1e-9);
    for v in &mut flux { *v /= maxv; }
    flux
}

/// Simple peak picking on spectral flux to get onset times (seconds).
pub fn onset_times(samples: &[f32], sr: u32, win: usize, hop: usize, thresh: f32) -> Vec<f32> {
    let flux = spectral_flux(samples, sr, win, hop);
    let mut times = Vec::new();
    let mut i = 1usize;
    while i + 1 < flux.len() {
        if flux[i] > thresh && flux[i] > flux[i-1] && flux[i] > flux[i+1] {
            let t = ((i * hop + win/2) as f32) / sr as f32;
            times.push(t);
            i += 2;
        } else {
            i += 1;
        }
    }
    times
}

/// YIN difference function for a frame.
fn yin_diff(frame: &[f32], tau_max: usize) -> Vec<f32> {
    let n = frame.len();
    let mut d = vec![0f32; tau_max+1];
    for tau in 1..=tau_max {
        let mut sum = 0f32;
        for i in 0..(n - tau) {
            let diff = frame[i] - frame[i+tau];
            sum += diff * diff;
        }
        d[tau] = sum;
    }
    d
}

/// Cumulative mean normalized difference (CMND) of YIN.
fn yin_cmnd(d: &[f32]) -> Vec<f32> {
    let mut cmnd = vec![0f32; d.len()];
    let mut running_sum = 0f32;
    cmnd[0] = 1.0;
    for tau in 1..d.len() {
        running_sum += d[tau];
        cmnd[tau] = if running_sum > 0.0 { d[tau] * (tau as f32) / running_sum } else { 1.0 };
    }
    cmnd
}

/// Estimate f0 for one frame with YIN. Returns (f0 Hz, confidence) or None.
fn yin_f0_frame(frame: &[f32], sr: u32, fmin: f32, fmax: f32, thresh: f32) -> Option<(f32, f32)> {
    let tau_min = (sr as f32 / fmax) as usize;
    let tau_max = (sr as f32 / fmin) as usize;
    if tau_max + 1 >= frame.len() { return None; }

    let d = yin_diff(frame, tau_max);
    let cmnd = yin_cmnd(&d);

    // find first minimum below threshold
    let mut tau = None;
    for t in tau_min..=tau_max {
        if cmnd[t] < thresh {
            // parabolic interpolation around t to refine
            let t0 = (t as isize - 1).max(1) as usize;
            let t2 = (t + 1).min(tau_max);
            let a = cmnd[t0];
            let b = cmnd[t];
            let c = cmnd[t2];
            let denom = a - 2.0*b + c;
            let mut t_adj = t as f32;
            if denom.abs() > 1e-9 {
                t_adj = t as f32 + 0.5 * (a - c) / denom;
            }
            tau = Some((t_adj, 1.0 - b));
            break;
        }
    }
    tau.map(|(t_adj, conf)| ((sr as f32) / t_adj.max(1.0), conf.clamp(0.0, 1.0)))
}

/// Track monophonic f0 across the whole signal (frame-wise). Returns (times_sec, f0_hz, confidence) vectors.
pub fn yin_track(samples: &[f32], sr: u32, win: usize, hop: usize, fmin: f32, fmax: f32, thresh: f32) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut times = Vec::new();
    let mut f0s = Vec::new();
    let mut confs = Vec::new();
    let mut frame = vec![0f32; win];
    let w = hann(win);

    for start in frames_indices(samples.len(), win, hop) {
        for i in 0..win { frame[i] = samples[start + i] * w[i]; }
        let center = (start + win/2) as f32 / sr as f32;
        if let Some((f0, conf)) = yin_f0_frame(&frame, sr, fmin, fmax, thresh) {
            times.push(center);
            f0s.push(f0);
            confs.push(conf);
        } else {
            times.push(center);
            f0s.push(0.0);
            confs.push(0.0);
        }
    }
    (times, f0s, confs)
}

/// Convenience: extract onsets (sec) and a crude monophonic melody (frame-wise f0).
pub struct MelodyExtraction {
    pub times: Vec<f32>,
    pub f0_hz: Vec<f32>,
    pub confidence: Vec<f32>,
    pub onsets_sec: Vec<f32>,
}

pub fn extract_melody_features(samples: &[f32], sr: u32) -> MelodyExtraction {
    let win = 2048;
    let hop = 256;
    let (times, f0, conf) = yin_track(samples, sr, win, hop, 80.0, 1000.0, 0.1);
    let onsets = onset_times(samples, sr, win, hop, 0.2);
    MelodyExtraction { times, f0_hz: f0, confidence: conf, onsets_sec: onsets }
}
