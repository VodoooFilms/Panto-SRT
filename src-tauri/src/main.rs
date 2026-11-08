#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod convert;
mod formats;
mod grouping;
mod media;
mod models;
mod parakeet;
mod plan;
mod process;
mod upscale;
mod vsr;
mod whisper;

use grouping::{Clip, GroupOptions, Word};
use process::ActiveSlot;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

struct AppState {
    language: Mutex<String>,
    cancelled: Arc<AtomicBool>,
    child_pid: Arc<Mutex<Option<u32>>>,
    download_cancelled: Arc<AtomicBool>,
    download_pid: Arc<Mutex<Option<u32>>>,
    download_in_progress: Arc<AtomicBool>,
    convert_process: ActiveSlot,
    upscale_process: ActiveSlot,
    vsr_download_cancelled: Arc<AtomicBool>,
    vsr_download_pid: Arc<Mutex<Option<u32>>>,
    vsr_download_in_progress: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            language: Mutex::new("es".to_string()),
            cancelled: Arc::new(AtomicBool::new(false)),
            child_pid: Arc::new(Mutex::new(None)),
            download_cancelled: Arc::new(AtomicBool::new(false)),
            download_pid: Arc::new(Mutex::new(None)),
            download_in_progress: Arc::new(AtomicBool::new(false)),
            convert_process: Arc::new(Mutex::new(None)),
            upscale_process: Arc::new(Mutex::new(None)),
            vsr_download_cancelled: Arc::new(AtomicBool::new(false)),
            vsr_download_pid: Arc::new(Mutex::new(None)),
            vsr_download_in_progress: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Prevents two model downloads from writing to the same `.part` file or
/// overwriting each other's cancellation PID.
struct DownloadGuard(Arc<AtomicBool>);

impl Drop for DownloadGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn begin_download(state: &AppState) -> Result<DownloadGuard, String> {
    if state.download_in_progress.swap(true, Ordering::SeqCst) {
        return Err(text(
            state,
            "Ya hay una descarga de modelo en curso.",
            "A model download is already in progress.",
        ));
    }
    Ok(DownloadGuard(Arc::clone(&state.download_in_progress)))
}

fn lang(state: &AppState) -> String {
    state.language.lock().unwrap().clone()
}

fn text(state: &AppState, es: &str, en: &str) -> String {
    if lang(state) == "en" {
        en.to_string()
    } else {
        es.to_string()
    }
}

fn engine_paths(app: &AppHandle) -> whisper::EnginePaths {
    whisper::EnginePaths {
        resource_dir: app.path().resource_dir().ok(),
        app_data_dir: app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from(".")),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscribeOptions {
    language: Option<String>,
    media_duration: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportOptions {
    timecode_base: Option<f64>,
    fps: Option<u32>,
}

#[tauri::command]
fn set_language(app: AppHandle, state: State<'_, AppState>, language: String) -> bool {
    if language != "es" && language != "en" {
        return false;
    }
    *state.language.lock().unwrap() = language.clone();
    if let Ok(menu) = build_menu(&app, &language) {
        let _ = app.set_menu(menu);
    }
    true
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubtitleFile {
    path: String,
    name: String,
}

fn extensions_without_dot<'a>(extensions: &'a [&'a str]) -> Vec<&'a str> {
    extensions
        .iter()
        .map(|extension| extension.trim_start_matches('.'))
        .collect()
}

fn convert_extensions(converter: &str) -> Vec<&str> {
    match converter {
        "mp4" => extensions_without_dot(media::VIDEO_EXTENSIONS),
        "mp3" => extensions_without_dot(media::VIDEO_EXTENSIONS)
            .into_iter()
            .chain(extensions_without_dot(media::AUDIO_EXTENSIONS))
            .collect(),
        "jpg" => extensions_without_dot(media::IMAGE_EXTENSIONS),
        _ => Vec::new(),
    }
}

#[tauri::command]
async fn select_convert_files(
    app: AppHandle,
    state: State<'_, AppState>,
    converter: String,
) -> Result<Vec<media::FileDetails>, String> {
    let extensions = convert_extensions(&converter);
    if extensions.is_empty() {
        return Ok(Vec::new());
    }
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title(text(
            &state,
            "Selecciona uno o varios archivos",
            "Select one or more files",
        ))
        .add_filter(
            text(&state, "Archivos compatibles", "Compatible files"),
            &extensions,
        )
        .pick_files(move |paths| {
            let _ = tx.send(paths);
        });
    let picked = rx.recv().map_err(|e| e.to_string())?;
    let mut details = Vec::new();
    for file_path in picked.unwrap_or_default() {
        let path = file_path.to_string();
        if media::is_convertible(&path) && Path::new(&path).exists() {
            details.push(media::file_details(&path)?);
        }
    }
    Ok(details)
}

#[tauri::command]
async fn inspect_convert_files(paths: Vec<String>) -> Result<Vec<media::FileDetails>, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<media::FileDetails>, String> {
        let mut details = Vec::new();
        for path in paths {
            if !media::is_convertible(&path) || !Path::new(&path).exists() {
                return Err("UNSUPPORTED".to_string());
            }
            details.push(media::file_details(&path)?);
        }
        Ok(details)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn select_convert_subtitle(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<SubtitleFile>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title(text(
            &state,
            "Selecciona subtítulos SRT",
            "Select SRT subtitles",
        ))
        .add_filter("SubRip", &["srt"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let picked = rx.recv().map_err(|e| e.to_string())?;
    Ok(picked.and_then(|file_path| {
        let path = file_path.to_string();
        convert::subtitle_file(&path).map(|valid| SubtitleFile {
            path: valid.to_string_lossy().into_owned(),
            name: Path::new(&path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        })
    }))
}

#[tauri::command]
fn inspect_convert_subtitle(path: String) -> Result<SubtitleFile, String> {
    convert::subtitle_file(&path)
        .map(|valid| SubtitleFile {
            name: Path::new(&path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: valid.to_string_lossy().into_owned(),
        })
        .ok_or_else(|| "UNSUPPORTED".to_string())
}

#[tauri::command]
async fn choose_convert_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title(text(
            &state,
            "Selecciona la carpeta de salida",
            "Select the output folder",
        ))
        .pick_folder(move |path| {
            let _ = tx.send(path);
        });
    let picked = rx.recv().map_err(|e| e.to_string())?;
    Ok(picked.map(|path| path.to_string()))
}

#[tauri::command]
async fn convert_media(
    app: AppHandle,
    state: State<'_, AppState>,
    request: convert::ConvertRequest,
) -> Result<convert::ConversionResult, String> {
    let lang = lang(&state);
    let slot = Arc::clone(&state.convert_process);
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        convert::convert(&app_handle, &slot, request, &lang)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn cancel_conversion(state: State<'_, AppState>) -> bool {
    process::cancel(&state.convert_process)
}

#[tauri::command]
async fn select_upscale_video(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<media::FileDetails>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title(text(&state, "Selecciona un vídeo", "Select a video"))
        .add_filter(
            text(&state, "Vídeos", "Videos"),
            &extensions_without_dot(media::VIDEO_EXTENSIONS),
        )
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let picked = rx.recv().map_err(|e| e.to_string())?;
    match picked {
        Some(file_path) => {
            let path = file_path.to_string();
            if !media::is_video(&path) || !Path::new(&path).exists() {
                return Err("UNSUPPORTED".to_string());
            }
            Ok(Some(media::file_details(&path)?))
        }
        None => Ok(None),
    }
}

#[tauri::command]
async fn inspect_upscale_video(path: String) -> Result<media::FileDetails, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !media::is_video(&path) || !Path::new(&path).exists() {
            return Err("UNSUPPORTED".to_string());
        }
        media::file_details(&path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn choose_upscale_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title(text(
            &state,
            "Selecciona la carpeta de salida",
            "Select the output folder",
        ))
        .pick_folder(move |path| {
            let _ = tx.send(path);
        });
    let picked = rx.recv().map_err(|e| e.to_string())?;
    Ok(picked.map(|path| path.to_string()))
}

#[tauri::command]
fn compute_upscale_plan(
    width: u32,
    height: u32,
    duration: f64,
    audio_kbps: u32,
    target: String,
    mode: String,
) -> plan::Plan {
    plan::compute_plan(width, height, duration, audio_kbps, &target, &mode)
}

#[tauri::command]
async fn start_upscale(
    app: AppHandle,
    state: State<'_, AppState>,
    request: upscale::UpscaleRequest,
) -> Result<upscale::UpscaleResult, String> {
    let slot = Arc::clone(&state.upscale_process);
    let app_handle = app.clone();
    let paths = engine_paths(&app);
    tauri::async_runtime::spawn_blocking(move || {
        upscale::start(&app_handle, &slot, &paths, request)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn cancel_upscale(state: State<'_, AppState>) -> bool {
    process::cancel(&state.upscale_process)
}

#[tauri::command]
fn list_upscale_models(app: AppHandle) -> Vec<vsr::UpscaleModelInfo> {
    vsr::list_models(&engine_paths(&app))
}

#[tauri::command]
fn set_upscale_engine(app: AppHandle, engine: String) -> Result<Vec<vsr::UpscaleModelInfo>, String> {
    let paths = engine_paths(&app);
    vsr::set_active_engine(&paths, &engine)?;
    Ok(vsr::list_models(&paths))
}

#[tauri::command]
fn delete_upscale_model(app: AppHandle, model_id: String) -> Result<Vec<vsr::UpscaleModelInfo>, String> {
    let paths = engine_paths(&app);
    vsr::delete_model(&paths, &model_id)?;
    Ok(vsr::list_models(&paths))
}

#[tauri::command]
async fn download_upscale_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model_id: String,
) -> Result<Vec<vsr::UpscaleModelInfo>, String> {
    let paths = engine_paths(&app);
    let model = vsr::def(&model_id).ok_or_else(|| format!("Modelo desconocido: {model_id}"))?;
    if state.vsr_download_in_progress.swap(true, Ordering::SeqCst) {
        return Err(text(
            &state,
            "Ya hay una descarga de modelo en curso.",
            "A model download is already in progress.",
        ));
    }
    struct Guard(Arc<AtomicBool>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let _guard = Guard(Arc::clone(&state.vsr_download_in_progress));
    state.vsr_download_cancelled.store(false, Ordering::SeqCst);
    let cancelled = Arc::clone(&state.vsr_download_cancelled);
    let download_pid = Arc::clone(&state.vsr_download_pid);
    let dir = whisper::models_dir(&paths);
    let total_all = vsr::total_size(model);
    let files: Vec<(String, String, u64, &'static str, u64)> = {
        let mut done_before = 0u64;
        model
            .files
            .iter()
            .map(|f| {
                let entry = (
                    f.url.to_string(),
                    dir.join(f.file).to_string_lossy().into_owned(),
                    f.size,
                    f.sha256,
                    done_before,
                );
                done_before += f.size;
                entry
            })
            .collect()
    };
    let app_progress = app.clone();
    let progress_id = model_id.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        for (url, destination, size, sha256, offset) in &files {
            let destination = PathBuf::from(destination);
            if destination.exists() {
                continue;
            }
            let app_progress = app_progress.clone();
            let progress_id = progress_id.clone();
            let offset = *offset;
            whisper::download_url_cancellable(
                url,
                &destination,
                Some(*size),
                Some(sha256),
                &|percent| {
                    let overall = ((offset + (size * percent as u64) / 100) * 100 / total_all) as i32;
                    let _ = app_progress.emit(
                        "upscale-model-download-progress",
                        json!({ "id": progress_id, "percent": overall }),
                    );
                },
                Some(&download_pid),
                Some(&cancelled),
            )?;
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(vsr::list_models(&paths))
}

#[tauri::command]
fn cancel_upscale_model_download(state: State<'_, AppState>) -> bool {
    state.vsr_download_cancelled.store(true, Ordering::SeqCst);
    whisper::kill_child(&state.vsr_download_pid);
    true
}

#[tauri::command]
fn ffmpeg_status() -> serde_json::Value {
    match media::ffmpeg_path() {
        Some(path) => json!({ "available": true, "path": path }),
        None => json!({ "available": false, "path": null }),
    }
}

#[tauri::command]
async fn select_media(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<media::FileDetails>, String> {
    let extensions: Vec<&str> = media::AUDIO_EXTENSIONS
        .iter()
        .chain(media::VIDEO_EXTENSIONS.iter())
        .map(|e| e.trim_start_matches('.'))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title(text(
            &state,
            "Selecciona un audio o video",
            "Select an audio or video file",
        ))
        .add_filter(
            text(&state, "Audio y video", "Audio and video"),
            &extensions,
        )
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let picked = rx.recv().map_err(|e| e.to_string())?;
    match picked {
        Some(file_path) => {
            let path = file_path.to_string();
            Ok(Some(media::file_details(&path)?))
        }
        None => Ok(None),
    }
}

#[tauri::command]
fn inspect_media(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<media::FileDetails, String> {
    if !media::is_supported(&file_path) || !Path::new(&file_path).exists() {
        return Err(text(
            &state,
            "El archivo no tiene un formato compatible.",
            "The file format is not supported.",
        ));
    }
    media::file_details(&file_path)
}

#[tauri::command]
fn list_models(app: AppHandle) -> Vec<models::ModelInfo> {
    models::list_models(&engine_paths(&app))
}

#[tauri::command]
fn set_active_model(app: AppHandle, model_id: String) -> Result<Vec<models::ModelInfo>, String> {
    let paths = engine_paths(&app);
    models::set_active_model(&paths, &model_id)?;
    Ok(models::list_models(&paths))
}

#[tauri::command]
fn delete_model(app: AppHandle, model_id: String) -> Result<Vec<models::ModelInfo>, String> {
    let paths = engine_paths(&app);
    models::delete_model(&paths, &model_id)?;
    Ok(models::list_models(&paths))
}

#[tauri::command]
async fn download_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model_id: String,
) -> Result<Vec<models::ModelInfo>, String> {
    let paths = engine_paths(&app);
    let model = models::def(&model_id).ok_or_else(|| format!("Modelo desconocido: {model_id}"))?;
    let _guard = begin_download(&state)?;
    let destination = whisper::models_dir(&paths).join(model.file);
    let url = model.url.to_string();
    let expected_sha256 = model.sha256;
    let total = model.size;
    let app_progress = app.clone();
    let progress_id = model_id.clone();
    state.download_cancelled.store(false, Ordering::SeqCst);
    let cancelled = Arc::clone(&state.download_cancelled);
    let download_pid = Arc::clone(&state.download_pid);
    tauri::async_runtime::spawn_blocking(move || {
        whisper::download_url_cancellable(
            &url,
            &destination,
            Some(total),
            Some(expected_sha256),
            &|percent| {
                let _ = app_progress.emit(
                    "model-download-progress",
                    json!({ "id": progress_id, "percent": percent }),
                );
            },
            Some(&download_pid),
            Some(&cancelled),
        )
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(models::list_models(&paths))
}

/// Stop an in-progress model download; the partial file is deleted.
#[tauri::command]
fn cancel_download(state: State<'_, AppState>) -> bool {
    state.download_cancelled.store(true, Ordering::SeqCst);
    whisper::kill_child(&state.download_pid);
    true
}

#[tauri::command]
async fn transcribe_media(
    app: AppHandle,
    state: State<'_, AppState>,
    file_path: String,
    options: Option<TranscribeOptions>,
) -> Result<whisper::TranscribeResult, String> {
    if !media::is_supported(&file_path) || !Path::new(&file_path).exists() {
        return Err(text(
            &state,
            "El archivo no tiene un formato compatible.",
            "The file format is not supported.",
        ));
    }
    let options = options.unwrap_or(TranscribeOptions {
        language: None,
        media_duration: None,
    });
    let paths = engine_paths(&app);
    let active = models::active_model(&paths);
    let active_def = models::def(&active).unwrap();
    let is_parakeet = active == models::PARAKEET_ID;
    // Whisper models keep their legacy ids ("turbo" / "small").
    let model_id = if active_def.whisper_id.is_empty() {
        "turbo".to_string()
    } else {
        active_def.whisper_id.to_string()
    };

    state.cancelled.store(false, Ordering::SeqCst);
    if !models::is_downloaded(&paths, active_def) {
        let _guard = begin_download(&state)?;
        let _ = app.emit(
            "transcription-status",
            json!({ "status": "model-download", "model": active }),
        );
        let app_progress = app.clone();
        let progress_model = active.clone();
        let emit_progress = move |percent: i32| {
            let _ = app_progress.emit(
                "transcription-progress",
                json!({ "phase": "model", "model": progress_model, "percent": percent }),
            );
        };
        let download_cancelled = Arc::clone(&state.cancelled);
        let download_pid = Arc::clone(&state.child_pid);
        if is_parakeet {
            let destination = whisper::models_dir(&paths).join(active_def.file);
            whisper::download_url_cancellable(
                active_def.url,
                &destination,
                Some(active_def.size),
                Some(active_def.sha256),
                &emit_progress,
                Some(&download_pid),
                Some(download_cancelled.as_ref()),
            )?;
        } else {
            whisper::download_model_cancellable(
                &paths,
                &model_id,
                &emit_progress,
                Some(&download_pid),
                Some(download_cancelled.as_ref()),
            )?;
        }
        if state.cancelled.load(Ordering::SeqCst) {
            return Err("cancelled".to_string());
        }
        let _ = app.emit(
            "transcription-status",
            json!({ "status": "model-ready", "model": active }),
        );
    }

    let cancelled = Arc::clone(&state.cancelled);
    let child_pid = Arc::clone(&state.child_pid);
    let language = options.language.unwrap_or_else(|| "auto".to_string());
    let duration = options.media_duration.unwrap_or(0.0);
    let app_progress = app.clone();
    let progress_model = active.clone();
    let parakeet_model = models::model_path(&paths, active_def);

    let _ = app.emit("transcription-status", json!({ "status": "starting" }));
    let result = tauri::async_runtime::spawn_blocking(move || {
        let on_progress = |percent: i32| {
            let _ = app_progress.emit(
                "transcription-progress",
                json!({ "phase": "transcribe", "model": progress_model, "percent": percent }),
            );
        };
        if is_parakeet {
            parakeet::transcribe(
                &paths,
                &file_path,
                &parakeet_model,
                &language,
                duration,
                &cancelled,
                &child_pid,
                &on_progress,
            )
        } else {
            whisper::transcribe(
                &paths,
                &file_path,
                &model_id,
                &language,
                duration,
                &cancelled,
                &child_pid,
                &on_progress,
            )
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    match result {
        Ok(value) => Ok(value),
        Err(message) => {
            if state.cancelled.load(Ordering::SeqCst) || message == "cancelled" {
                Err("cancelled".to_string())
            } else if message == "engine-missing" {
                Err(text(
                    &state,
                    "No se encontró el motor de transcripción en la app.",
                    "Transcription engine not found in the app.",
                ))
            } else if message.starts_with("model-missing:") {
                Err(text(
                    &state,
                    "El modelo Whisper aún no está descargado. Panto Studio lo descargará ahora.",
                    "The Whisper model is not downloaded yet. Panto Studio will download it now.",
                ))
            } else {
                Err(message)
            }
        }
    }
}

#[tauri::command]
fn cancel_transcription(app: AppHandle, state: State<'_, AppState>) -> bool {
    state.cancelled.store(true, Ordering::SeqCst);
    whisper::kill_child(&state.child_pid);
    let _ = app.emit("transcription-status", json!({ "status": "cancelling" }));
    true
}

#[tauri::command]
fn group_clips(words: Vec<Word>, options: Option<GroupOptions>) -> Vec<Clip> {
    let options = options.unwrap_or(GroupOptions {
        max_chars: None,
        max_duration: None,
        min_gap: None,
        min_sentence_break: None,
    });
    grouping::build_clips(&words, &options)
}

fn unique_output_path(source_path: &str, suffix: &str, extension: &str) -> PathBuf {
    let source = Path::new(source_path);
    let directory = source.parent().unwrap_or_else(|| Path::new("."));
    let name = source
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut candidate = directory.join(format!("{name}{suffix}{extension}"));
    let mut count = 2;
    while candidate.exists() {
        candidate = directory.join(format!("{name}{suffix}-{count}{extension}"));
        count += 1;
    }
    candidate
}

#[tauri::command]
fn export_subtitles(
    state: State<'_, AppState>,
    source_path: String,
    format: String,
    clips: Vec<Clip>,
    options: Option<ExportOptions>,
) -> Result<serde_json::Value, String> {
    if !media::is_supported(&source_path) || !Path::new(&source_path).exists() {
        return Err(text(
            &state,
            "El archivo no tiene un formato compatible.",
            "The file format is not supported.",
        ));
    }
    if clips.iter().all(|c| c.text.trim().is_empty()) {
        return Err("No hay clips para exportar.".to_string());
    }
    let options = options.unwrap_or(ExportOptions {
        timecode_base: None,
        fps: None,
    });
    let offset = options.timecode_base.unwrap_or(0.0);
    let fps = options.fps.unwrap_or(25).clamp(1, 120);

    let output_path = match format.as_str() {
        "fcpxml" => {
            let suffix = if lang(&state) == "es" {
                "-subtítulos"
            } else {
                "-subtitles"
            };
            let path = unique_output_path(&source_path, suffix, ".xml");
            std::fs::write(
                &path,
                formats::build_fcpxml(&clips, fps, offset, &source_path),
            )
            .map_err(|e| e.to_string())?;
            path
        }
        "srt" => {
            let path = unique_output_path(&source_path, "", ".srt");
            std::fs::write(&path, formats::build_srt(&clips, offset)).map_err(|e| e.to_string())?;
            path
        }
        other => return Err(format!("Formato desconocido: {other}")),
    };
    let bytes = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(json!({ "outputPath": output_path.to_string_lossy(), "bytes": bytes }))
}

#[tauri::command]
fn show_in_folder(app: AppHandle, file_path: String) -> Result<(), String> {
    if Path::new(&file_path).exists() {
        app.opener()
            .reveal_item_in_dir(&file_path)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn build_menu(app: &AppHandle, language: &str) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
    let es = language != "en";
    let pkg = app.package_info();
    let about = PredefinedMenuItem::about(
        app,
        Some(if es { "Acerca de Panto Studio" } else { "About Panto Studio" }),
        Some(AboutMetadata {
            name: Some("Panto Studio".to_string()),
            version: Some(pkg.version.to_string()),
            copyright: Some("Copyright © 2026 Panto Studio".to_string()),
            credits: Some(
                if es {
                    "Panto Studio convierte video, audio e imágenes, escala a 4K y transcribe voz a subtítulos. Todo ocurre 100% en tu equipo, sin cuentas ni nube."
                } else {
                    "Panto Studio converts video, audio and images, upscales to 4K, and turns voice into subtitles. Everything runs 100% on your device, with no accounts or cloud."
                }
                .to_string(),
            ),
            ..Default::default()
        }),
    )?;
    let app_menu = Submenu::with_items(
        app,
        "Panto Studio",
        true,
        &[
            &about,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(
                app,
                Some(if es {
                    "Ocultar Panto Studio"
                } else {
                    "Hide Panto Studio"
                }),
            )?,
            &PredefinedMenuItem::hide_others(
                app,
                Some(if es { "Ocultar otras" } else { "Hide Others" }),
            )?,
            &PredefinedMenuItem::show_all(app, Some(if es { "Mostrar todo" } else { "Show All" }))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(
                app,
                Some(if es {
                    "Salir de Panto Studio"
                } else {
                    "Quit Panto Studio"
                }),
            )?,
        ],
    )?;
    let edit_menu = Submenu::with_items(
        app,
        if es { "Edición" } else { "Edit" },
        true,
        &[
            &PredefinedMenuItem::undo(app, Some(if es { "Deshacer" } else { "Undo" }))?,
            &PredefinedMenuItem::redo(app, Some(if es { "Rehacer" } else { "Redo" }))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some(if es { "Cortar" } else { "Cut" }))?,
            &PredefinedMenuItem::copy(app, Some(if es { "Copiar" } else { "Copy" }))?,
            &PredefinedMenuItem::paste(app, Some(if es { "Pegar" } else { "Paste" }))?,
            &PredefinedMenuItem::select_all(
                app,
                Some(if es { "Seleccionar todo" } else { "Select All" }),
            )?,
        ],
    )?;
    let window_menu = Submenu::with_items(
        app,
        if es { "Ventana" } else { "Window" },
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some(if es { "Minimizar" } else { "Minimize" }))?,
            &PredefinedMenuItem::maximize(app, Some("Zoom"))?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                "front",
                if es {
                    "Traer todo al frente"
                } else {
                    "Bring All to Front"
                },
                true,
                None::<&str>,
            )?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .setup(|app| {
            let menu = build_menu(app.handle(), "es")?;
            app.set_menu(menu)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_language,
            select_media,
            inspect_media,
            transcribe_media,
            cancel_transcription,
            group_clips,
            export_subtitles,
            show_in_folder,
            list_models,
            set_active_model,
            delete_model,
            download_model,
            cancel_download,
            select_convert_files,
            inspect_convert_files,
            select_convert_subtitle,
            inspect_convert_subtitle,
            choose_convert_folder,
            convert_media,
            cancel_conversion,
            select_upscale_video,
            inspect_upscale_video,
            choose_upscale_folder,
            compute_upscale_plan,
            start_upscale,
            cancel_upscale,
            list_upscale_models,
            set_upscale_engine,
            delete_upscale_model,
            download_upscale_model,
            cancel_upscale_model_download,
            ffmpeg_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running Panto Studio");
}
