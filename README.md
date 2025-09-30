# Xformed

A creative transformation toolkit that converts between **text, images, and audio** using interpretable feature extraction and procedural synthesis.  
Semantic and structural features (sentiment, color, spectral statistics, etc.) are mapped into musical or visual domains.

---

## 📌 Description & Purpose
**Xformed** is an experimental playground for *cross-modal transformations*:  
- **Text → Audio**: generate melodies (WAV + MIDI JSON) based on linguistic structure and sentiment.  
- **Image → Audio**: generate soundscapes based on color palettes, contrast, and complexity.  
- **Audio → Metrics (JSON)**: extract interpretable features (RMS, spectral centroid, entropy, tempo, etc.).  

The goal is to **bridge content modalities** with lightweight, explainable mappings — without heavy ML models.

---

## ⚡ Features
- **Text → Audio**  
  - Sentiment → major/minor scale.  
  - Syllables, words → tempo and note density.  
  - Punctuation & entropy → rhythm variety and dynamics.  
  - Procedural synth with multiple oscillators (sine, saw, square).  
  - Optional percussions with fills & ghost notes.  

- **Image → Audio**  
  - Hue → tonal center.  
  - Brightness/contrast → tempo & dynamics.  
  - Edge density & variance → rhythm and harmonic complexity.  

- **Audio → JSON Metrics**  
  - Loudness: RMS, peak, crest factor.  
  - Spectrum: centroid, rolloff, flatness, bandwidth.  
  - Entropy: spectral & amplitude.  
  - Onset rate, tempo estimation.  
  - Fundamental frequency stats (mean, std, voiced ratio).  

- **CLI-first design**: everything runs locally; no external server.  

---

## 🚀 Installation

### Requirements
- [Rust](https://www.rust-lang.org/tools/install) (≥ 1.70 recommended)  
- Cargo (comes with Rust)  

### Build
Clone and build the workspace:

```bash
git clone https://github.com/IoanU/Xformed.git
cd Xformed
cargo build --workspace
```

---

## ▶️ Usage

Main entry point is the CLI:

```bash
cargo run -p xformed-cli -- <COMMAND> [OPTIONS]
```

### Text → Audio
Convert text into melody:

```bash
cargo run -p xformed-cli -- text-to-audio --name hello "this is a test phrase"
```

Outputs:
- `outputs/hello.wav` – rendered audio.  
- `outputs/hello.midi.json` – MIDI timeline in JSON.  

### Image → Audio
Convert an image (base64 or file) into audio:

```bash
cargo run -p xformed-cli -- image-to-audio --name sunset ./examples/sunset.png
```

Outputs:
- `outputs/sunset.wav`  
- `outputs/sunset.midi.json`  

### Audio → Features
Extract metrics from a WAV:

```bash
cargo run -p xformed-cli -- audio-to-json ./examples/drumloop.wav
```

Outputs JSON with RMS, spectral features, entropy, tempo, etc.

---

## 📂 Project Structure
- `crates/text-features` – text analysis (syllables, entropy, sentiment).  
- `crates/visual-features` – image analysis (color, edges, brightness).  
- `crates/melody-core` – core MIDI timeline representation.  
- `crates/melody-synth` – procedural audio synthesis engine.  
- `crates/converters` – mapping text/image/audio → artifacts.  
- `crates/xformed-cli` – command-line interface.  
- `services/api` – optional service layer.  
