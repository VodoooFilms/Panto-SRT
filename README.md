<p align="center">
  <img src="assets/icons/panto-srt.png" width="110" alt="Panto Studio">
</p>

<h1 align="center">Panto Studio</h1>

<p align="center">
  <b>The local & private video studio</b><br>
  Convert media · Upscale video with AI · Automatic subtitles — in one app, fully offline.<br>
  No cloud. No accounts. Your files never leave your machine.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/macOS-13.4%2B-lightgrey.svg" alt="macOS 13.4+">
  <img src="https://img.shields.io/badge/tauri-2-orange.svg" alt="Tauri 2">
  <img src="https://img.shields.io/badge/rust-red.svg" alt="Rust">
  <img src="https://img.shields.io/badge/AI-Real--ESRGAN-blueviolet.svg" alt="Real-ESRGAN AI">
</p>

<p align="center">
  <a href="README.es.md">Español</a>
</p>

---

## Three tabs, zero cloud

**Convertir** — the good parts of [Pantoraya](https://github.com/VodoooFilms/pantoraya):
batch-convert video/audio/images to MP4, MP3 or JPG with quality or space-saver
profiles, drag & drop, per-file output list.

**Upscale** — the good parts of [Panto Upscaler](https://github.com/VodoooFilms/Pantoupscaler),
now with real AI:

| Engine | Speed (832×480 input) | Ships in the app |
|---|---|---|
| Lanczos (FFmpeg) | instant | — |
| **Real-ESRGAN ×2** | ~0.2 s/frame | 1.2 MB |
| **Real-ESRGAN ×4** | <1 s/frame | 1.2 MB |

`realesr-animevideov3` is trained for **animation and video**, so cartoons,
stylized footage and gameplay come out clean instead of blurry. Targets
720p / 1080p / 1440p / 4K with black bars (fit), crop or stretch — and the
original audio is always muxed back. The AI engines are bundled inside the app:
nothing to download, nothing to configure.

**Subtítulos** — automatic, word-accurate subtitles: whisper.cpp + NVIDIA
Parakeet, SRT and FCPXML export ready for DaVinci Resolve.

All of it runs **100% on your machine**.

---

## Highlights

- Native **Rust + Tauri 2** app — small binary, no Electron
- **Spanish & English** UI with a one-click toggle
- **AI upscaling** via [Real-ESRGAN](https://github.com/xinntao/Real-ESRGAN)
  (BSD-3) on ncnn + Metal — models included, GPU-accelerated
- **Lanczos fallback** for instant, resource-light scaling
- Two **subtitle engines**: Whisper (Turbo / Small) and NVIDIA
  **Parakeet TDT 0.6B v3** (download on demand, managed in-app)
- Model manager for the subtitle engines: switch, download with progress, delete
- Light / dark theme, minimal Pantoraya-style UI

---

> ☕ Panto Studio is free and open source (MIT). If it saves you time, consider
> [inviting me a coffee on PayPal](https://paypal.me/antoniomartinez75)
> (`@antoniomartinez75`). Totally optional — but deeply appreciated!

---

## Getting started

Requires the Rust toolchain, the Tauri CLI (`cargo install tauri-cli --version '^2'`),
and **ffmpeg** on `PATH`.

```bash
# 0. Clone the repo
git clone https://github.com/VodoooFilms/PantoStudio.git
cd PantoStudio

# 1. Install Node helpers and prepare the bundled engines
npm install
npm run prepare:whisper   # whisper.cpp
npm run prepare:nemo      # NeMo/Parakeet (macOS Apple Silicon)
npm run prepare:esrgan    # realesrgan-ncnn-vulkan + weights (~27 MB, pinned SHA-256)

# 2. Run in development
npm start

# 3. Build the app bundle for the current platform
npm run build
```

Verify and test:

```bash
npm run check   # cargo check the Rust backend
npm test        # subtitle engine tests (grouping, SRT, FCPXML, whisper JSON)
```

Requirements: **macOS 13.4+ (Apple Silicon)**, ~2–6 GB free RAM. Internet is
only needed once, for the subtitle models (they download into the app's data
folder on first use). The upscaler's and converter's engines are already bundled.

---

## Upscaling with AI

1. Drop a video into **Upscale** (MP4, MOV, MKV, WEBM, AVI).
2. Pick the target (720p–4K), the fit mode and the engine from **Modelos**.
3. Escalar. Progress shows the exact frame being processed and a live ETA.

The pipeline is: FFmpeg extracts PNG frames → `realesrgan-ncnn-vulkan` (GPU) →
FFmpeg encodes the target with the original audio. Cancel at any moment;
partial files are always cleaned up.

For real-time animation content, Real-ESRGAN delivers crisp line art where a
plain Lanczos pass softens details.

---

## Subtitles

1. Drop an audio or video file — transcription starts with the **Turbo** model
   and auto language detection.
2. Review the text inline; **Re-group** after retiming.
3. Export **SRT** or **FCPXML** (`File > Import > Subtitle…` /
   `File > Import > Timeline > FCP XML…` in Resolve). Clips start at
   `00:00:00,000`, so they land in sync on a timeline at zero.

| Model | Download | Engine | Notes |
|---|---|---|---|
| Whisper Large v3 Turbo | ~1.6 GB | whisper.cpp | Default, best balance |
| Whisper Small | ~466 MB | whisper.cpp | Low-end machines |
| Parakeet TDT 0.6B v3 | ~681 MB | NeMo (Q8) | Fastest on Apple Silicon, ES/EN (macOS only) |

---

## Project structure

```
src-tauri/             Rust backend (Tauri 2)
  src/
    main.rs            window, menu, commands
    convert.rs         Pantoraya conversion profiles (MP4/MP3/JPG)
    upscale.rs         upscaler pipeline (Lanczos + Real-ESRGAN + audio mux)
    plan.rs            target/fit math and ffmpeg filters
    vsr.rs             upscale engine catalog + settings
    whisper.rs         whisper.cpp engine (spawn, parse, progress)
    parakeet.rs        NeMo/Parakeet engine wrapper
    models.rs          subtitle model catalog
    media.rs           audio/video introspection
    grouping.rs        word timestamps → subtitle clips
    formats.rs         SRT + FCPXML exporters
  tauri.conf.json      window + bundle config
src/renderer/          UI (vanilla JS, no framework)
scripts/
  prepare-whisper.js   build whisper engine
  prepare-nemo.js      download the Parakeet runtime (macOS ARM)
  prepare-esrgan.js    download the bundled Real-ESRGAN runner + weights
build/esrgan/          bundled AI engine (inside the .app)
```

---

## License

- Code: [MIT](./LICENSE)
- AI upscaler: [Real-ESRGAN ncnn Vulkan](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan) (MIT) + weights [Real-ESRGAN](https://github.com/xinntao/Real-ESRGAN) (BSD-3-Clause)
- Subtitles engine: [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (MIT)
- Whisper models: [OpenAI](https://github.com/openai/whisper) (MIT)

Third-party licenses are collected in [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).
