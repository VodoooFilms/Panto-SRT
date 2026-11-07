<p align="center">
  <img src="assets/icons/panto-srt.png" width="110" alt="Panto SRT">
</p>

<h1 align="center">Panto SRT</h1>

<p align="center">
  <b>Local & private automatic subtitles</b><br>
  Turn any audio or video into a timed subtitle file for video editing.<br>
  No cloud. No accounts. Your files never leave your machine.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/macOS-13.4%2B-lightgrey.svg" alt="macOS 13.4+">
  <img src="https://img.shields.io/badge/tauri-2-orange.svg" alt="Tauri"> <img src="https://img.shields.io/badge/rust-1.98-red.svg" alt="Rust">
  <img src="https://img.shields.io/badge/language-es%20%2F%20en-brightgreen.svg" alt="Spanish / English">
</p>

<p align="center">
  <a href="README.es.md">Español</a>
</p>

---

## Why Panto SRT?

Writing subtitles by ear is slow. Panto SRT transcribes your audio locally using
[whisper.cpp](https://github.com/ggml-org/whisper.cpp), groups the words into
clean subtitle clips, and exports them as **SRT** or **FCPXML** — ready for
DaVinci Resolve.

- **100% local & private** — no accounts, no cloud, no uploads
- **English & Spanish** out of the box (auto-detected)
- **Precise timing** from actual whisper word timestamps
- **Clean clips** — character length, max duration and min pause auto-balance
- **Re-group on the fly** after you edit text
- **Export SRT** (drag straight onto a Resolve subtitle track) or **FCPXML**
  (import as a title-generator timeline), 25 fps / timecode from zero
- Minimalist, Pantoraya-style UI with a **light / dark** toggle
- **Model manager** — switch engines in one tap, download on demand, delete to free space
- **Two ASR engines**: Whisper (Turbo / Small) and NVIDIA **Parakeet TDT 0.6B v3** (Q8, ~4× faster on Apple Silicon)
- Native **Rust + Tauri 2** app — a 30 MB binary, no Electron

---

<p align="center">
  <svg width="42" height="42" viewBox="0 0 24 24" fill="none" stroke="#b87333" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" style="opacity:.8">
    <path d="M17 8h1a4 4 0 0 1 0 8h-1"/>
    <path d="M3 8h14v9a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4Z"/>
    <line x1="6" y1="2" x2="6" y2="4"/>
    <line x1="10" y1="2" x2="10" y2="4"/>
    <line x1="14" y1="2" x2="14" y2="4"/>
  </svg>
</p>

> ☕ Panto SRT is free and open source. If it saves you time, consider
> [inviting me a coffee on PayPal](https://paypal.me/antoniomartinez75)
> (`@antoniomartinez75`). Totally optional — but deeply appreciated!

---

## Getting started

Requires the Rust toolchain, the Tauri CLI (`cargo install tauri-cli --version '^2'`),
and **ffmpeg** on `PATH` (needed to decode MP4/MOV/other containers and to probe
their duration; plain WAV/MP3/FLAC work without it).

```bash
# 0. Install the small Node helper used by the Windows engine download
npm install

# 1. Build the local engines (first time only — ~0.1s if already present)
npm run prepare:whisper   # whisper.cpp
npm run prepare:nemo      # NeMo/Parakeet (macOS Apple Silicon only; skipped elsewhere)

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

Requirements: **macOS 13.4+ (Apple Silicon)** or **Windows 10/11 x64**,
~2–6 GB free RAM depending on the model, and internet only the first time
(the Whisper model downloads on demand into the app's data folder).
The **Parakeet** engine is currently macOS Apple Silicon only; Windows builds
ship the Whisper engines.

---

## Making subtitles

1. Drop an audio or video file (MP3, M4A, WAV, MP4, MOV…).
2. Transcription starts automatically with the **Turbo** model and
   **auto language detection**, grouping clips at sensible defaults
   (42 chars, 3.5 s max, 0.2 s min pause).
3. Review and fix the text inline; hit **Re-group** if you retimed manually.
4. **Export SRT** or **Export FCPXML**.

In Resolve: `File > Import > Subtitle…` (SRT) or
`File > Import > Timeline > FCP XML…`. Clips always start at `00:00:00,000`, so
they land in sync on a timeline that starts at zero.

---

## Models

Models download on demand the first time you generate subtitles, and are
managed from the **Modelos / Models** card in the header: tap to switch,
download with progress (stoppable), or delete to free space.

| Model               | Download | Engine      | Notes                          |
|---------------------|----------|-------------|--------------------------------|
| Whisper Large v3 Turbo | ~1.6 GB | whisper.cpp | Default, best balance          |
| Whisper Small       | ~466 MB  | whisper.cpp | Low-end machines               |
| Parakeet TDT 0.6B v3 | ~681 MB | NeMo (Q8)   | Fastest on Apple Silicon, ES/EN (macOS only) |

---

## Project structure

```
src-tauri/             Rust backend (Tauri 2)
  tauri.conf.json      window + bundle config
  src/
    main.rs            window, menu, commands
    whisper.rs         whisper.cpp engine (spawn, parse, progress)
    parakeet.rs        NeMo/Parakeet engine wrapper
    models.rs          model catalog + active model settings
    media.rs           audio/video file introspection
    grouping.rs        word timestamps → subtitle clips
    formats.rs         SRT + FCPXML exporters
  capabilities/        Tauri permissions
src/
  renderer/
    index.html         minimal UI
    styles.css
    app.js
    tauri-bridge.js    window.pantoSRT bridge over Tauri IPC
scripts/
  prepare-whisper.js   build engine + (optional) models
  prepare-nemo.js      download the Parakeet runtime (macOS ARM)
```

---

## License

- Code: [MIT](./LICENSE)
- Engine: [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (MIT)
- Whisper models: [OpenAI](https://github.com/openai/whisper) (MIT)
- See [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)