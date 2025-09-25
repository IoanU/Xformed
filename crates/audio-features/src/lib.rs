// crates/audio-features/src/lib.rs
pub mod decode;
pub use decode::decode_wav_to_mono_f32;

use serde::{Serialize, Deserialize};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct F0Stats {
    pub mean_hz: f32,
    pub std_hz: f32,
    pub voiced_ratio: f32, // [0,1]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFeatures {
    // Amplitude
    pub rms: f32,
    pub peak: f32,
    pub crest_factor: f32,

    // Time-domain
    pub zcr: f32,              // zero-crossings/sec
    pub onset_rate: f32,       // onsets/sec
    pub tempo_bpm: f32,

    // Spectral (frame-avg)
    pub spectral_centroid_hz: f32,
    pub spectral_rolloff85_hz: f32,
    pub spectral_rolloff95_hz: f32,
    pub spectral_flatness: f32, // [0,1] ~ geometric/arith mean
    pub spectral_bandwidth_hz: f32,
    pub spectral_entropy: f32,   // [0,1]

    // Amplitude entropy
    pub amplitude_entropy: f32,  // [0,1]

    // F0 (YIN-lite)
    pub f0: F0Stats,
}

pub struct FeatureExtractor {
    pub target_sr: u32,     // e.g. 22050
    pub frame_size: usize,  // e.g. 2048
    pub hop_size: usize,    // e.g. 512
}

impl FeatureExtractor {
    pub fn new(target_sr: u32, frame_size: usize, hop_size: usize) -> Self {
        Self { target_sr, frame_size, hop_size }
    }

    /// Dacă ai deja decode → mono f32 @sr, folosește direct asta.
    pub fn analyze_mono(&self, mono: &[f32], sr: u32) -> Result<AudioFeatures> {
        use rustfft::{FftPlanner, num_complex::Complex};
        use anyhow::bail;

        if mono.is_empty() || sr == 0 { bail!("empty signal"); }

        // 1) Basic amp stats
        let mut sum2 = 0.0f64;
        let mut peak = 0.0f32;
        for &x in mono { 
            let ax = x.abs();
            if ax > peak { peak = ax; }
            sum2 += (x as f64)*(x as f64);
        }
        let rms = (sum2 / mono.len() as f64).sqrt() as f32;
        let crest = if rms > 0.0 { peak / rms } else { 0.0 };

        // 2) ZCR (per sec)
        let mut zc = 0usize;
        for w in mono.windows(2) {
            let (a,b) = (w[0], w[1]);
            if (a >= 0.0 && b < 0.0) || (a < 0.0 && b >= 0.0) { zc += 1; }
        }
        let zcr = (zc as f32) * (sr as f32) / (mono.len().saturating_sub(1).max(1) as f32);

        // Framing
        let n = mono.len();
        let fs = self.frame_size;
        let hop = self.hop_size;
        let n_frames = if n < fs { 0 } else { 1 + (n - fs)/hop };
        if n_frames == 0 {
            return Ok(AudioFeatures {
                rms, peak, crest_factor: crest, zcr,
                onset_rate: 0.0, tempo_bpm: 0.0,
                spectral_centroid_hz: 0.0, spectral_rolloff85_hz: 0.0,
                spectral_rolloff95_hz: 0.0, spectral_flatness: 0.0,
                spectral_bandwidth_hz: 0.0, spectral_entropy: 0.0,
                amplitude_entropy: 0.0,
                f0: F0Stats{mean_hz:0.0,std_hz:0.0,voiced_ratio:0.0},
            });
        }

        // Hann window
        let mut window = vec![0.0f32; fs];
        for i in 0..fs {
            window[i] = 0.5 - 0.5 * (2.0*std::f32::consts::PI*(i as f32)/(fs as f32)).cos();
        }

        // FFT
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fs);
        let bin2hz = |k: usize| (k as f32) * (sr as f32) / (fs as f32);

        let mut centroid_sum = 0.0f64;
        let mut roll85_sum = 0.0f64;
        let mut roll95_sum = 0.0f64;
        let mut flatness_sum = 0.0f64;
        let mut bandwidth_sum = 0.0f64;
        let mut spec_entropy_sum = 0.0f64;

        // Onset (spectral flux)
        let mut prev_mag = vec![0.0f32; fs/2+1];
        let mut flux_vals = Vec::with_capacity(n_frames);

        for fi in 0..n_frames {
            let start = fi*hop;
            let frame = &mono[start..start+fs];

            // Window + copy to complex buffer
            let mut buf: Vec<Complex<f32>> = frame.iter()
                .zip(&window)
                .map(|(x,w)| Complex{ re: x*w, im: 0.0 })
                .collect();

            fft.process(&mut buf);

            // Power spectrum (one-sided)
            let mut mag = vec![0.0f32; fs/2+1];
            for k in 0..=fs/2 {
                let c = buf[k];
                mag[k] = (c.re*c.re + c.im*c.im).sqrt();
            }

            // Spectral centroid / bandwidth (weighted by magnitude)
            let mut wsum = 0.0f64;
            let mut ksum = 0.0f64;
            for k in 0..=fs/2 {
                let m = mag[k] as f64;
                wsum += m;
                ksum += m * (k as f64);
            }
            let centroid_bin = if wsum>0.0 { ksum/wsum } else { 0.0 };
            let centroid_hz = centroid_bin as f32 * (sr as f32)/(fs as f32);
            centroid_sum += centroid_hz as f64;

            // Bandwidth (2nd central moment around centroid)
            let mut var = 0.0f64;
            for k in 0..=fs/2 {
                let m = mag[k] as f64;
                let d = (k as f64) - centroid_bin;
                var += m * d*d;
            }
            let bw_bin = if wsum>0.0 { (var/wsum).sqrt() } else { 0.0 };
            let bw_hz = bw_bin as f32 * (sr as f32)/(fs as f32);
            bandwidth_sum += bw_hz as f64;

            // Rolloff (85%, 95%)
            let mut csum = 0.0f64;
            let total: f64 = mag.iter().map(|&m| m as f64).sum();
            let thr85 = 0.85 * total;
            let thr95 = 0.95 * total;
            let mut r85 = 0usize;
            let mut r95 = 0usize;
            if total > 0.0 {
                for k in 0..=fs/2 {
                    csum += mag[k] as f64;
                    if r85==0 && csum>=thr85 { r85 = k; }
                    if r95==0 && csum>=thr95 { r95 = k; break; }
                }
            }
            roll85_sum += bin2hz(r85) as f64;
            roll95_sum += bin2hz(r95) as f64;

            // Flatness (geo/arith)
            let eps = 1e-12f64;
            let geo = mag.iter().fold(0.0f64, |acc, &m| acc + (m as f64 + eps).ln());
            let geo = (geo / (mag.len() as f64)).exp();
            let arith = (total + eps) / (mag.len() as f64);
            let flat = (geo/arith).clamp(0.0, 1.0);
            flatness_sum += flat;

            // Spectral entropy (normalize to pmf, H/logN)
            let mut p = vec![0.0f64; mag.len()];
            let total_p: f64 = mag.iter().map(|&m| m as f64).sum::<f64>() + eps;
            for (i,&m) in mag.iter().enumerate() {
                p[i] = (m as f64) / total_p;
            }
            let h = -p.iter().map(|&pi| if pi>0.0 { pi*(pi.ln()) } else { 0.0 }).sum::<f64>();
            let h_norm = (h / (mag.len() as f64).ln()).clamp(0.0, 1.0);
            spec_entropy_sum += h_norm;

            // Flux (ReLU of mag diff)
            let mut flux = 0.0f32;
            for k in 0..mag.len() {
                let d = (mag[k] - prev_mag[k]).max(0.0);
                flux += d;
            }
            flux_vals.push(flux);
            prev_mag = mag;
        }

        // Onset rate (pe sec): prag adaptiv pe flux
        let mean_flux = if !flux_vals.is_empty() {
            flux_vals.iter().sum::<f32>() / (flux_vals.len() as f32)
        } else { 0.0 };
        let thr = mean_flux * 1.5; // simplu
        let mut onsets = 0usize;
        for &f in &flux_vals {
            if f > thr { onsets += 1; }
        }
        let secs = n as f32 / sr as f32;
        let onset_rate = if secs>0.0 { onsets as f32 / secs } else { 0.0 };

        // Tempo (autocorrelare pe flux → bpm peak în [50..200])
        let bpm = {
            if flux_vals.len() < 4 { 0.0 }
            else {
                let mut ac = vec![0.0f32; flux_vals.len()];
                for lag in 1..flux_vals.len() {
                    let mut s = 0.0f32;
                    let mut c = 0usize;
                    let mut i = lag;
                    while i < flux_vals.len() {
                        s += flux_vals[i] * flux_vals[i - lag];
                        c += 1; i += 1;
                    }
                    ac[lag] = if c>0 { s/(c as f32) } else { 0.0 };
                }
                // map lag->bpm
                let fps = (sr as f32) / (hop as f32);
                let mut best_bpm = 0.0f32;
                let mut best_val = 0.0f32;
                for lag in 1..ac.len() {
                    let period_sec = (lag as f32)/fps;
                    if period_sec <= 0.0 { continue; }
                    let cand_bpm = 60.0/period_sec;
                    if cand_bpm >= 50.0 && cand_bpm <= 200.0 && ac[lag] > best_val {
                        best_val = ac[lag];
                        best_bpm = cand_bpm;
                    }
                }
                best_bpm
            }
        };

        // Amplitude entropy (histogram 64 bins)
        let amp_entropy = {
            let bins = 64usize;
            let mut hist = vec![0usize; bins];
            let mut total = 0usize;
            for &x in mono {
                // map [-1,1] -> [0,bins)
                let v = ((x * 0.5 + 0.5) * (bins as f32 - 1.0)).clamp(0.0, bins as f32 - 1.0);
                hist[v as usize] += 1;
                total += 1;
            }
            if total == 0 { 0.0 } else {
                let total_f = total as f64;
                let h: f64 = hist.iter().map(|&c| {
                    if c==0 { 0.0 } else {
                        let p = c as f64 / total_f;
                        -p * p.ln()
                    }
                }).sum();
                (h / (bins as f64).ln()) as f32
            }
        };

        // F0 (YIN-lite pe fereastra lungă, voiced ratio via energy + ACF peak)
        let f0 = {
            let win = (sr/50).max(1024) as usize; // ~20ms+
            let step = hop.max(256);
            let mut f0s = Vec::new();
            let mut voiced = 0usize;
            let mut i = 0usize;
            while i + win <= mono.len() {
                let fr = &mono[i..i+win];
                let mean: f32 = fr.iter().copied().sum::<f32>()/(fr.len() as f32);
                let energy: f32 = fr.iter().map(|&x|(x-mean)*(x-mean)).sum::<f32>()/(fr.len() as f32);
                // simple acf
                let mut best_p = 0usize;
                let mut best_v = 0.0f32;
                for p in (sr/400).max(2) as usize .. (sr/60) as usize {
                    let mut s = 0.0f32; let mut c = 0usize;
                    let mut j = p;
                    while j<fr.len() { s += (fr[j]-mean)*(fr[j-p]-mean); c+=1; j+=1; }
                    if c>0 { s /= c as f32; }
                    if s > best_v { best_v = s; best_p = p; }
                }
                // voiced heuristic
                if energy > 1e-4 && best_v > 1e-4 {
                    voiced += 1;
                    let hz = sr as f32 / best_p.max(1) as f32;
                    if hz.is_finite() { f0s.push(hz); }
                }
                i += step;
            }
            let (mean, std, vr) = if f0s.is_empty() {
                (0.0, 0.0, 0.0)
            } else {
                let m = f0s.iter().sum::<f32>()/(f0s.len() as f32);
                let v = f0s.iter().map(|&x|(x-m)*(x-m)).sum::<f32>()/(f0s.len() as f32);
                (m, v.sqrt(), (voiced as f32)/((mono.len()/step).max(1) as f32))
            };
            F0Stats{ mean_hz: mean, std_hz: std, voiced_ratio: vr.clamp(0.0,1.0) }
        };

        Ok(AudioFeatures{
            rms, peak, crest_factor: crest, zcr,
            onset_rate, tempo_bpm: bpm,
            spectral_centroid_hz: (centroid_sum/n_frames as f64) as f32,
            spectral_rolloff85_hz: (roll85_sum/n_frames as f64) as f32,
            spectral_rolloff95_hz: (roll95_sum/n_frames as f64) as f32,
            spectral_flatness: (flatness_sum/n_frames as f64) as f32,
            spectral_bandwidth_hz: (bandwidth_sum/n_frames as f64) as f32,
            spectral_entropy: (spec_entropy_sum/n_frames as f64) as f32,
            amplitude_entropy: amp_entropy,
            f0,
        })
    }
}
