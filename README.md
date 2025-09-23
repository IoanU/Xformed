# Xformed

A creative transformation tool that converts between **text, audio, and images** using rule-based feature extraction and procedural synthesis.  
The project demonstrates how semantic and structural features (sentiment, color, spectral statistics, etc.) can be mapped into musical or visual domains.

---

## 📌 Description & Purpose
**Xformed** is an experimental playground for "cross-modal transformations":  
- From **text** → generate melodies (WAV + MIDI).  
- From **images** → generate soundscapes based on color and visual features.  
- From **audio** → extract metrics (RMS, tempo, entropy, etc.) or map features into visuals.  

The main goal is to **bridge content modalities** (text, audio, visual) with interpretable metrics and lightweight synthesis, without relying on heavy machine learning models.

---

## ⚡ Features
- **Text → Audio**:  
  - Sentiment analysis → choose minor/major.  
  - Syllable count → number of notes.  
  - Procedural synth with sine, saw, or square waves.  
- **Image → Audio**:  
  - Color → tonality and pitch space.  
  - Brightness/contrast → tempo & dynamics.  
- **Audio → Metrics**:  
  - RMS, peak, crest factor.  
  - Spectral centroid, rolloff, flatness, bandwidth.  
  - Entropy (amplitude & spectral).  
  - Onset rate & tempo estimation.  
  - f0 stats (mean, std, voiced ratio).  
- **Audio → Visual / Text**: map spectral energy and entropy to palettes and descriptors.  
- **CLI-first design**: no server needed, run transformations locally.  

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

The main entry point is the CLI:

```bash
cargo run -p xformed-cli -- <COMMAND> [OPTIONS]
```

### Text → Audio
Convert a text phrase into melody:

```bash
cargo run -p xformed-cli -- text-to-audio --text "un apus rece peste blocuri"
```

Generates:
- `outputs/out.wav`  
- `outputs/out.mid`

### Interactive Text Input
```bash
cargo run -p xformed-cli -- text-to-audio
```
Type your text and press Enter.

Input:  
```
"un apus rece peste blocuri, noapte caldă dar un pic tristă"
```

Output:  
- Melody in D minor, tempo ~85 BPM, 11 notes.  
- Files: `outputs/out.wav`, `outputs/out.mid`.

---

### Image → Audio
```bash
cargo run -p xformed-cli -- image-to-audio inputs/dark_gray_peisage.png
```
Generates:
- `outputs/out_from_image.wav`  
- `outputs/out_from_image.mid`

### Options
Global options:
- `--instrument` (sine | saw | square)  
- `--mood` (auto | major | minor)  
- `--tempo` (BPM, optional)  
- `--jumpiness` (0.0–1.0, melodic contour)  
- `--out-dir` (default: `outputs/`)  

---

## 📂 Project Structure

```
Xformed/
├── Cargo.toml                 # Workspace manifest
├── crates/
│   ├── audio-features/        # Extracts audio metrics
│   ├── text-features/         # Extracts text metrics
│   ├── visual-features/       # Extracts image metrics
│   ├── melody-core/           # MIDI generation
│   ├── melody-synth/          # WAV synthesis (sine/saw/square)
│   ├── converters/            # Cross-modal converters
│   ├── api/                   # Optional Axum HTTP API
│   └── xformed-cli/           # CLI frontend
├── examples/                  # Sample images / results (kept in repo)
└── outputs/                   # Generated artifacts (ignored or local)
