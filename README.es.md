<p align="center">
  <img src="assets/icons/panto-srt.png" width="110" alt="Panto SRT">
</p>

<h1 align="center">Panto SRT</h1>

<p align="center">
  <b>Subtítulos automáticos locales y privados</b><br>
  Convierte cualquier audio o video en un archivo de subtítulos sincronizados para edición de video.<br>
  Sin nube. Sin cuentas. Tus archivos nunca salen de tu máquina.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/macOS-13.4%2B-lightgrey.svg" alt="macOS 13.4+">
  <img src="https://img.shields.io/badge/tauri-2-orange.svg" alt="Tauri"> <img src="https://img.shields.io/badge/rust-1.98-red.svg" alt="Rust">
  <img src="https://img.shields.io/badge/language-es%20%2F%20en-brightgreen.svg" alt="Español / Inglés">
</p>

<p align="center">
  <a href="README.md">English</a>
</p>

---

## Por qué Panto SRT

Repasar audio y escribir subtítulos a mano es lento. Panto SRT ejecuta el motor
open-source [whisper.cpp](https://github.com/ggml-org/whisper.cpp) **en local**,
lee los timestamps reales a nivel de palabra y los reagrupa en clips de
subtítulos limpios, listos para DaVinci Resolve.

- **100 % local y privado** — sin cuentas, sin nube, sin subidas
- **Español e inglés** desde el primer día (detección automática)
- **Tiempos precisos** desde los timestamps reales de whisper
- **Clips limpios** — longitud de caracteres, duración máxima y pausa mínima se equilibran solas
- **Reagrupar al vuelo** tras corregir el texto
- **Exporta SRT** (arrástralo a una pista de subtítulos de Resolve) o **FCPXML**
  (impórtalo como timeline de títulos), 25 fps / timecode desde cero
- Interfaz mínima estilo Pantoraya con modo **claro / oscuro**
- **Gestor de modelos**: cambia de motor con un toque, descarga bajo demanda y borra para liberar espacio
- **Dos motores ASR**: Whisper (Turbo / Small) y **Parakeet TDT 0.6B v3** de NVIDIA (Q8, ~4× más rápido en Apple Silicon)
- App nativa en **Rust + Tauri 2**: binario de 30 MB, sin Electron

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

> ☕ Panto SRT es gratuito y open source. Si te ahorra tiempo, considera
> [invitarme un café por PayPal](https://paypal.me/antoniomartinez75)
> (`@antoniomartinez75`). Totalmente opcional — ¡pero muy apreciado!

---

## Empezar

Requiere el toolchain de Rust, el CLI de Tauri (`cargo install tauri-cli --version '^2'`)
y **ffmpeg** en el `PATH` (necesario para decodificar MP4/MOV y otros
contenedores y para leer su duración; WAV/MP3/FLAC funcionan sin él).

```bash
# 0. Instala el pequeño helper de Node que usa la descarga del motor en Windows
npm install

# 1. Compila los motores locales (solo la primera vez — ~0,1 s si ya existen)
npm run prepare:whisper   # whisper.cpp
npm run prepare:nemo      # NeMo/Parakeet (solo macOS Apple Silicon; se omite en otros sistemas)

# 2. Ejecuta en modo desarrollo
npm start

# 3. Construye la app para la plataforma actual
npm run build
```

Verifica y prueba:

```bash
npm run check   # cargo check del backend Rust
npm test        # tests del motor de subtítulos (grouping, SRT, FCPXML, JSON de Whisper)
```

Requisitos: **macOS 13.4+ (Apple Silicon)** o **Windows 10/11 x64**,
~2–6 GB de RAM libre según el modelo, e internet solo la primera vez
(el modelo Whisper se descarga bajo demanda a la carpeta de datos de la app).
El motor **Parakeet** actualmente es solo para macOS Apple Silicon; las
versiones de Windows incluyen los motores Whisper.

---

## Generar subtítulos

1. Arrastra un audio o video (MP3, M4A, WAV, MP4, MOV…).
2. La transcripción arranca automáticamente con el modelo **Turbo** y
   **detección automática de idioma**, agrupando clips con los límites por
   defecto (42 caracteres, 3,5 s máx, 0,2 s de pausa mínima).
3. Revisa y corrige el texto en el editor; usa **Reagrupar** si ajustaste los
   tiempos manualmente.
4. **Exporta SRT** o **Exporta FCPXML**.

En Resolve: `File > Import > Subtitle…` (SRT) o
`File > Import > Timeline > FCP XML…`. Los clips siempre parten de
`00:00:00,000`, por lo que se importan sincronizados sobre una timeline que
empieza en cero.

---

## Modelos

Los modelos se descargan bajo demanda la primera vez que generas subtítulos
y se gestionan desde la tarjeta **Modelos** de la cabecera: tocar para
cambiar, descargar con progreso (detenible) o borrar para liberar espacio.

| Modelo                 | Descarga | Motor       | Notas                            |
|------------------------|----------|-------------|----------------------------------|
| Whisper Large v3 Turbo | ~1,6 GB  | whisper.cpp | Por defecto, mejor equilibrio    |
| Whisper Small          | ~466 MB  | whisper.cpp | Equipos modestos                 |
| Parakeet TDT 0.6B v3   | ~681 MB  | NeMo (Q8)   | El más rápido en Apple Silicon (solo macOS) |

---

## Estructura del proyecto

```
src-tauri/             backend Rust (Tauri 2)
  tauri.conf.json      ventana + bundle
  src/
    main.rs            ventana, menú, comandos
    whisper.rs         motor whisper.cpp (spawn, parseo, progreso)
    parakeet.rs        motor NeMo/Parakeet
    models.rs          catálogo de modelos + modelo activo
    media.rs           inspección de archivos de audio/video
    grouping.rs        timestamps por palabra → clips de subtítulos
    formats.rs         exportadores SRT + FCPXML
  capabilities/        permisos Tauri
src/
  renderer/
    index.html         interfaz mínima
    styles.css
    app.js
    tauri-bridge.js    puente window.pantoSRT sobre IPC de Tauri
scripts/
  prepare-whisper.js   compila motor + (opcional) modelos
  prepare-nemo.js      descarga el runtime Parakeet (macOS ARM)
```

---

## Licencias

- Código: [MIT](./LICENSE)
- Motor: [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (MIT)
- Modelos Whisper: [OpenAI](https://github.com/openai/whisper) (MIT)
- Ver [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)