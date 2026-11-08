<p align="center">
  <img src="assets/icons/panto-srt.png" width="110" alt="Panto Studio">
</p>

<h1 align="center">Panto Studio</h1>

<p align="center">
  <b>El estudio de video local y privado</b><br>
  Convertir medios · Escalar video con IA · Subtítulos automáticos — en una sola app, 100% offline.<br>
  Sin nube. Sin cuentas. Tus archivos nunca salen de tu máquina.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/macOS-13.4%2B-lightgrey.svg" alt="macOS 13.4+">
  <img src="https://img.shields.io/badge/tauri-2-orange.svg" alt="Tauri 2">
  <img src="https://img.shields.io/badge/rust-red.svg" alt="Rust">
  <img src="https://img.shields.io/badge/AI-Real--ESRGAN-blueviolet.svg" alt="Real-ESRGAN AI">
</p>

<p align="center">
  <a href="README.md">English</a>
</p>

---

## Tres pestañas, cero nube

**Convertir** — lo mejor de [Pantoraya](https://github.com/VodoooFilms/pantoraya):
convierte por lotes video/audio/imágenes a MP4, MP3 o JPG con perfiles de
calidad o ahorro de espacio, con arrastrar y soltar.

**Upscale** — lo mejor de [Panto Upscaler](https://github.com/VodoooFilms/Pantoupscaler),
ahora con IA de verdad:

| Motor | Velocidad (entrada 832×480) | Ya viene en la app |
|---|---|---|
| Lanczos (FFmpeg) | instantáneo | — |
| **Real-ESRGAN ×2** | ~0,2 s/frame | 1,2 MB |
| **Real-ESRGAN ×4** | <1 s/frame | 1,2 MB |

`realesr-animevideov3` está entrenado para **animación y video**: caricaturas,
material estilizado y gameplays salen limpios y nítidos en vez de borrosos.
Destinos de 720p / 1080p / 1440p / 4K con barras negras (ajustar), recorte o
estirado — y el audio original siempre vuelve pegado. Los motores de IA van
dentro de la app: nada que descargar, nada que configurar.

**Subtítulos** — subtítulos automáticos precisos a nivel de palabra:
whisper.cpp + NVIDIA Parakeet, exportación a SRT y FCPXML lista para
DaVinci Resolve.

Todo corre **100% en tu equipo**.

---

## Destacados

- App nativa **Rust + Tauri 2** — binario liviano, sin Electron
- Interfaz en **español e inglés** con un click
- **Escalado con IA** vía [Real-ESRGAN](https://github.com/xinntao/Real-ESRGAN)
  (BSD-3) sobre ncnn + Metal — modelos incluidos, acelerado por GPU
- **Respaldo Lanczos** para escalar al instante sin recursos
- Dos **motores de subtítulos**: Whisper (Turbo / Small) y NVIDIA
  **Parakeet TDT 0.6B v3** (descarga bajo demanda, administrado en la app)
- Administrador de modelos de subtítulos: cambiar, descargar con progreso, borrar
- Tema claro / oscuro, interfaz estilo Pantoraya

---

> ☕ Panto Studio es gratis y open source (MIT). Si te ahorra tiempo, considera
> [invitarme un café por PayPal](https://paypal.me/antoniomartinez75)
> (`@antoniomartinez75`). Completamente opcional — pero se agradece muchísimo!

---

## Empezar

Requiere el toolchain de Rust, la CLI de Tauri (`cargo install tauri-cli --version '^2'`)
y **ffmpeg** en el `PATH`.

```bash
# 0. Clona el repo
git clone https://github.com/VodoooFilms/PantoStudio.git
cd PantoStudio

# 1. Instala los helpers de Node y prepara los motores incluidos
npm install
npm run prepare:whisper   # whisper.cpp
npm run prepare:nemo      # NeMo/Parakeet (macOS Apple Silicon)
npm run prepare:esrgan    # realesrgan-ncnn-vulkan + pesos (~27 MB, SHA-256 fijado)

# 2. corre en desarrollo
npm start

# 3. compila el bundle de app para la plataforma actual
npm run build
```

Verificar y probar:

```bash
npm run check   # cargo check del backend Rust
npm test        # tests del motor de subtítulos (agrupación, SRT, FCPXML, JSON de whisper)
```

Requisitos: **macOS 13.4+ (Apple Silicon)**, ~2–6 GB de RAM libre. Internet solo
se necesita una vez, para los modelos de subtítulos (se descargan a la carpeta
de datos de la app en el primer uso). Los motores del escalador y el conversor
ya vienen incluidos.

---

## Escalar con IA

1. Arrastra un video a **Upscale** (MP4, MOV, MKV, WEBM, AVI).
2. Elige destino (720p–4K), modo de encuadre y motor desde **Modelos**.
3. Escalar. El progreso muestra el frame exacto en curso y un restante estimado.

El pipeline es: FFmpeg extrae frames PNG → `realesrgan-ncnn-vulkan` (GPU) →
FFmpeg codifica el destino con el audio original. Puedes cancelar cuando
quieres; los archivos parciales siempre se limpian.

Para contenido animado, Real-ESRGAN entrega arte de líneas crisp donde un
escala simple Lanczos embaza los detalles.

---

## Subtítulos

1. Arrastra un audio o video — la transcripción arranca con el modelo **Turbo**
   y detección automática de idioma.
2. Corrige el texto en línea; usa **Re-agrupar** si ajustas tiempos.
3. Exporta **SRT** o **FCPXML** (`File > Import > Subtitle…` /
   `File > Import > Timeline > FCP XML…` en Resolve). Los clips arrancan en
   `00:00:00,000`, así que caen en sincronía en una línea de tiempo desde cero.

| Modelo | Descarga | Motor | Notas |
|---|---|---|---|
| Whisper Large v3 Turbo | ~1,6 GB | whisper.cpp | Por defecto, mejor balance |
| Whisper Small | ~466 MB | whisper.cpp | Máquinas de gama baja |
| Parakeet TDT 0.6B v3 | ~681 MB | NeMo (Q8) | El más rápido en Apple Silicon, ES/EN (solo macOS) |

---

## Estructura del proyecto

```
src-tauri/             Backend Rust (Tauri 2)
  src/
    main.rs            ventana, menú, comandos
    convert.rs         perfiles de conversión Pantoraya (MP4/MP3/JPG)
    upscale.rs         pipeline del escalador (Lanczos + Real-ESRGAN + audio)
    plan.rs            matemática de destino/encuadre y filtros ffmpeg
    vsr.rs             catálogo de motores de escala + ajustes
    whisper.rs         motor whisper.cpp (spawn, parseo, progreso)
    parakeet.rs        envoltorio de NeMo/Parakeet
    models.rs          catálogo de modelos de subtítulos
    media.rs           introspección de audio/video
    grouping.rs        timestamps de palabra → clips de subtítulo
    formats.rs         exportadores SRT + FCPXML
  tauri.conf.json      ventana + configuración de bundle
src/renderer/          UI (JS vanilla, sin framework)
scripts/
  prepare-whisper.js   compila el motor whisper
  prepare-nemo.js      descarga el runtime de Parakeet (macOS ARM)
  prepare-esrgan.js    descarga el runner de Real-ESRGAN + pesos incluidos
build/esrgan/          motor de IA incluido (dentro del .app)
```

---

## Licencia

- Código: [MIT](./LICENSE)
- Escalador IA: [Real-ESRGAN ncnn Vulkan](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan) (MIT) + pesos [Real-ESRGAN](https://github.com/xinntao/Real-ESRGAN) (BSD-3-Clause)
- Motor de subtítulos: [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (MIT)
- Modelos Whisper: [OpenAI](https://github.com/openai/whisper) (MIT)

Las licencias de terceros están recogidas en [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).
