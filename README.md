# Xformed Studio

**Feel it in another form.** A creative transformation toolkit that converts **between text, images and audio**, using interpretable feature extraction and procedural synthesis. The studio turns sources into music, generative art and emotional writing. It runs entirely in the browser through WebAssembly, with an optional native Rust API and CLI.

Bring a line you cannot forget, an image you love, or a sound that stays with you. Give it another form. Keep the result, make another take, or build a small collection.

## What you can make

| Start with | Make |
| --- | --- |
| Words | A multi-instrument composition or original fractal artwork |
| An image | A composition, a poem, or a short prose vignette |
| Sound | Fractal artwork, a poem, or a prose vignette |
| Any source | Feature analysis, in a separate analysis mode |

Art has three visual systems, each with four composition families. **Nebula** explores plumes, orbits, tides and rifts; **Bloom** explores corollas, Julia archipelagos, phyllotaxis and folded coral; **Contour** explores relief, strata, interference and cellular fields. Source-derived scale, warping, aspect, centers, frequencies and palette harmonies change the geometry within each family. Successive takes cycle through all four families. Artwork exports at 1280 × 832.

Writing follows six emotional registers: luminous, tender, restless, wistful, nocturnal and serene. The engine uses a local mood grammar, with poetic imagery and variations derived from the source. It produces imaginative interpretations, rather than object captions, speech transcripts or statements about a person's feelings. Its vocabulary is finite. English poems and prose are supported; word sentiment cues include English and Romanian.

Music derives its identity from the whole source. Seven composition characters choose different ensemble roles, meters (3/4, 4/4, 5/4 or 6/8), phrase lengths, diatonic modes, functional chord chains and Euclidean pulse distributions. Motifs recur with inversion, sequencing and call-and-response; voice-led chords, cadences and evolving sections give the score a form. Ensembles use up to seven roles and 12 procedural timbres: piano, electric piano, marimba, bell, guitar, strings, pad, flute, organ, brass, bass and drums. New takes change musical structure, not just instrumentation. WAV and multi-track MIDI share the same score.

## Open the studio

### Web app / GitHub Pages

[Open Xformed](https://ioanu.github.io/Xformed/). A modern browser with WebAssembly and module-worker support is required. All conversions run on the visitor's device: original file bytes are not sent to a conversion server. No account, API key, local server or installation is needed. Choose **Use example** under Image or Sound to try a locally generated source.

To build the static site locally, install stable Rust and Node 22 or newer:

```sh
rustup target add wasm32-unknown-unknown
node scripts/build-web.mjs
node scripts/wasm-smoke.mjs
node scripts/serve-web.mjs
```

Open http://127.0.0.1:8082/Xformed/. The test server serves static files only, under the same subpath as Pages. `dist/` contains the complete publishable site. The build script downloads the matching official wasm-bindgen 0.2.130 CLI, verifies its published SHA-256 checksum and caches it under `target/web-tools`. `WASM_BINDGEN` can point to an existing matching executable; `CARGO` can point to a non-default Cargo executable.

The `Build and deploy Xformed` GitHub Actions workflow runs Rust regression tests, builds the static site, tests the real WebAssembly worker and ZIP exports, then deploys `dist/` on pushes to `main`. Enable **Settings → Pages → Source → GitHub Actions** for a fork. Pull requests build and test without deploying.

The worker owns the Rust engine and its memory; canceling a conversion terminates that worker and a later conversion starts a fresh one. Image, audio, text, MIDI, analysis and exports use the same Rust conversion code as the native edition. Rendering speed and memory capacity depend on the visitor's device. The initial engine download requires a connection; this release does not install an offline service worker.

### Windows package

Extract the Windows ZIP and double-click **Start-Studio.cmd**. It starts the server and opens http://127.0.0.1:8080. No account, API key, Rust or Node installation is required for the packaged executables. **Stop-Studio.cmd** stops that copy of the server.

### From source

Install current stable Rust, then:

```sh
cargo run --release -p api
```

The release build matters for large photographs and audio analysis. The interface is embedded in the executable and does not need a frontend build or external assets.

## The creative workflow

1. Select Words, Image or Sound. Type a thought or drop a file.
2. Choose Music, Artwork, Writing or Analysis. Pick a visual system or writing form where applicable.
3. Render. Browse the result, listen, copy the words, or export individual files or a ZIP.
4. Use **New take** for a different composition from the same source.

The last six creative results are saved in **this browser's local IndexedDB collection**. Original source file bytes are not stored in that collection. A short source label is retained: a filename, or up to 70 characters of typed words. Reload to revisit results; reopening a saved creation does not automatically reload its original source. Browser storage can be cleared by the user or browser; download results you want to keep. The interface handles unavailable local storage without blocking generation.

The interface uses two main columns for source and result. A horizontal route diagram above the transformation display keeps all four ports and their labels readable on desktop and mobile. The collection opens on demand. Anime.js 4.5.0 drives SVG motion paths, the persistent transformation display and coordinated reveal timelines; it is bundled locally under its MIT license, without a CDN dependency. The large display cycles between text lines, image meshes, audio waveforms and analysis bars for all nine supported routes. It stays visible and animated after generation; images shape the mesh and audio playback drives the waveform. A persistent Motion toggle and reduced-motion preferences provide a static view when requested. Keyboard focus states, accessible labels, responsive layouts, engine status, cancellation and recoverable errors support the workflow.

**Creative views show the creation.** Technical metrics and raw feature JSON appear only when Feature analysis is selected. Music's key, tempo and score are part of the musical result.

## Exports

- Art: PNG; creation metadata is included in Download all.
- Writing: UTF-8 TXT and clipboard copy; metadata is included in Download all.
- Music: 44.1 kHz 16-bit stereo WAV, SMF format-1 MIDI (conductor + active instrument tracks), and complete score JSON including musical identity.
- Analysis: feature JSON and clipboard copy.

MIDI includes the score's time signature, General MIDI programs, drum channel 10 when used, volume/pan controllers and section markers. MIDI timbres depend on the playback synthesizer; source-dependent room and echo effects belong to the WAV mix. WAV has a short effect tail. Instrument sounds are synthesized, not sampled acoustic instruments. Very short pieces contain fewer musical sections.

## CLI

```sh
cargo run --release -p xformed-cli -- text-to-audio --text "A little hope, a bright morning" --duration 40 --name morning
cargo run --release -p xformed-cli -- text-to-image --text "I miss you in the quiet" --visual-style bloom
cargo run --release -p xformed-cli -- text-to-image --text "I miss you in the quiet" --visual-style contour --variation 1
cargo run --release -p xformed-cli -- image-to-text --input photograph.jpg --writing-style vignette
cargo run --release -p xformed-cli -- image-to-audio --input photograph.jpg --duration 40
cargo run --release -p xformed-cli -- audio-to-image --input recording.mp3 --visual-style nebula
cargo run --release -p xformed-cli -- audio-to-text --input recording.wav --writing-style poem
cargo run --release -p xformed-cli -- text-features --text "A small thought"
cargo run --release -p xformed-cli -- image-features --input photograph.jpg
cargo run --release -p xformed-cli -- audio-features --input recording.mp3
```

Text commands read standard input when `--text` is omitted. `--out-dir`, `--name`, `--duration`, `--visual-style`, `--writing-style` and `--variation` are global options. Default output filenames are `<source>_to_<destination>.*` in `outputs/`. Names are sanitized; files with matching names are overwritten. The same source, options and variation reproduce the same output within the same build/runtime; small floating-point differences between platforms can change source-derived choices. Variation changes art, writing and music.

## API

`GET /health` returns `ok`. `POST /convert` accepts:

```json
{
  "from": "text",
  "to": "image",
  "options": { "image_style": "bloom", "variation": 0 },
  "payload": { "kind": "Text", "text": "I am learning to love the light again" }
}
```

`from`: `text`, `image`, `audio`. `to`: a different modality, or `json`. Image and audio payloads use `ImageBase64` / `AudioBase64` with a `data_b64` field. The historical AudioBase64 name also accepts supported compressed audio. Options may be omitted. Other options are `target_seconds`, `text_style` (`poem` or `vignette`), plus the existing text duration bounds. Image styles: `nebula`, `contour`, `bloom`.

The response is an `artifacts` array tagged by `type`: `WavBase64`, `MidiBase64`, `MidiJsonBase64`, `PngBase64`, `Text` (a UTF-8 `text` field), `Json` (a `data` field), or `CreativeInfo` (title, mood, description and seed). CreativeInfo is presentation/reproducibility metadata, not a feature report. Creative routes no longer append technical Json artifacts. API clients should ignore artifact types they do not use.

The service binds to loopback; `XFORMED_ADDR` overrides the address. Two conversions run at once; further requests receive 429. Invalid conversions return JSON errors with HTTP 400. Invalid JSON/request shapes use Axum's 4xx rejection responses. Canceling a browser request stops waiting for its result; already-started server work may finish before a worker slot is available again.

Limits: 10 MB uploads in the studio, 16 million base64 characters via API, 64 megapixels per decoded image, 100 KB text, 180 seconds of audio input. Images: JPG/PNG/WebP in the interface. Audio: WAV, MP3, FLAC, OGG and supported M4A/AAC variants through Symphonia; availability depends on the file's actual codec. WAV supports PCM 8/16/24/32-bit and float32. Musical duration: 1–180 seconds (studio: 3–90).

## Verify

```sh
cargo test --workspace
cargo build --release --workspace
# With the server running; Node 18+ only needed for these checks:
node scripts/smoke.mjs
node scripts/zip-test.mjs
node scripts/wasm-smoke.mjs
node scripts/diversity-qa.mjs http://127.0.0.1:8080
# Optional, user-specific acceptance corpus in this workstation's Downloads:
node scripts/downloads-qa.mjs
node scripts/downloads-qa.mjs --wasm
```

The Downloads acceptance script reads seven selected existing files, writes derived review samples to the surrounding outputs/MVP-review directory, and leaves originals untouched. It is an explicit local QA utility, not a portable fixture suite. Core tests do not depend on personal files.

Regression coverage includes sentiment negation and Romanian cues, creative/analysis separation, deterministic images, distinct styles and variations, valid image/WAV/MIDI output, all 12 voices, arrangement timing, corrupt audio, tiny images, hue wrapping and invalid input. The HTTP smoke suite covers all six creative directions, three feature routes, stereo WAV, multi-track MIDI and server recovery.

The diversity audit compares eight fixed texts. It checks melodic interval sequences (independent of key), quantized onset differences (ignoring tiny timing changes), harmonic sequences, repeatability, late-source changes and musical takes. Visual unit tests compare grayscale geometry, not just file hashes or color changes. These are structural regression checks, not a claim that any two outputs must be aesthetically unique.

Motion references: [Anime.js timeline](https://animejs.com/documentation/timeline/) and [SVG motion paths](https://animejs.com/documentation/svg/createmotionpath/). Rhythmic reference: [Toussaint, The Euclidean Algorithm Generates Traditional Musical Rhythms](https://cgm.cs.mcgill.ca/~godfried/teaching/dm-reading-assignments/Euclidean-Rhythms.pdf). The pulse generator uses evenly distributed pulses with source-derived rotations and density.

## Workspace

- `crates/converters`: mood grammar, fractal engine, cross-modal mappings, validation
- `crates/melody-core`, `melody-synth`: musical scores, arrangement, instruments and stereo synthesis
- `crates/text-features`, `visual-features`, `audio-features`: interpretable extraction and input decoding
- `crates/xformed-cli`: terminal interface
- `crates/xformed-web`: WebAssembly entry point for the shared conversion engine
- `services/api`: local server and embedded studio
- `services/api/static/runtime.js`, `worker.js`: native/browser engine transport and cancelable worker
- `scripts/build-web.mjs`, `.github/workflows/pages.yml`: static build and GitHub Pages deployment

This is a creative MVP with browser and native editions. It provides an end-to-end product workflow; it is not a semantic vision/transcription model, hosted multi-user service or claim of commercial validation.
