//! Panto Upscaler's Lanczos pipeline, adapted to the shared Panto Studio state.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::media;
use crate::plan::{self, Mode, Plan};
use crate::process::{self, ActiveProcess, ActiveSlot};
use crate::vsr;
use crate::whisper::EnginePaths;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleRequest {
    pub input_path: String,
    pub target_id: String,
    pub mode: String,
    pub output_directory: Option<String>,
    /// "lanczos" (default) or a RelayVSR model id ("relay-x2" / "relay-x4").
    pub engine: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleResult {
    pub output_path: String,
    pub output_name: String,
    pub output_bytes: u64,
    pub input_bytes: u64,
    pub thumbnail: Option<String>,
    pub elapsed_seconds: f64,
    pub plan: Plan,
}

fn build_arguments(
    input: &Path,
    output: &Path,
    filter: &str,
    has_audio: bool,
    audio_can_be_copied: bool,
) -> Vec<String> {
    let mut args = vec![
        "-hide_banner".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        input.to_string_lossy().into_owned(),
        "-map".to_string(),
        "0:v:0".to_string(),
    ];

    if has_audio {
        args.push("-map".to_string());
        args.push("0:a?".to_string());
        if audio_can_be_copied {
            args.push("-c:a".to_string());
            args.push("copy".to_string());
        } else {
            args.push("-c:a".to_string());
            args.push("aac".to_string());
            args.push("-b:a".to_string());
            args.push("192k".to_string());
        }
    } else {
        args.push("-an".to_string());
    }

    args.extend([
        "-vf".to_string(),
        filter.to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "veryslow".to_string(),
        "-crf".to_string(),
        "14".to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        "-map_metadata".to_string(),
        "0".to_string(),
        "-map_chapters".to_string(),
        "0".to_string(),
        "-metadata:s:v:0".to_string(),
        "rotate=0".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-nostats".to_string(),
        output.to_string_lossy().into_owned(),
    ]);

    args
}

fn unique_output_path(input: &Path, label: &str, directory: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("video");
    let base = format!("{stem}_{}", label.to_lowercase());

    let mut candidate = directory.join(format!("{base}.mp4"));
    let mut count = 2;
    while candidate.exists() {
        candidate = directory.join(format!("{base}-{count}.mp4"));
        count += 1;
    }
    candidate
}

fn audio_can_be_copied(codec: Option<&str>) -> bool {
    matches!(codec, Some("aac" | "mp3" | "ac3" | "eac3" | "alac"))
}

/// Runs the upscale. Blocking: call from `spawn_blocking`.
pub fn start(
    app: &AppHandle,
    slot: &ActiveSlot,
    paths: &EnginePaths,
    request: UpscaleRequest,
) -> Result<UpscaleResult, String> {
    let input = PathBuf::from(&request.input_path);
    if !media::is_video(&request.input_path) {
        return Err("UNSUPPORTED".to_string());
    }
    let input_metadata = std::fs::metadata(&input).map_err(|_| "UNSUPPORTED".to_string())?;
    if !input_metadata.is_file() {
        return Err("UNSUPPORTED".to_string());
    }

    {
        let guard = slot.lock().map_err(|_| "STATE".to_string())?;
        if guard.is_some() {
            return Err("BUSY".to_string());
        }
    }

    let probe = media::probe_media(&request.input_path);
    if probe.width == 0 || probe.height == 0 {
        return Err("UNSUPPORTED".to_string());
    }

    let engine = request.engine.as_deref().unwrap_or(vsr::LANCZOS_ID);
    if engine != vsr::LANCZOS_ID {
        return start_ai(
            app,
            slot,
            paths,
            &request,
            &input,
            &input_metadata,
            &probe,
            engine,
        );
    }

    let target = plan::target_by_id(&request.target_id);
    let mode = Mode::parse(&request.mode);
    let upscale_plan = plan::compute_plan(
        probe.width,
        probe.height,
        probe.duration,
        probe.audio_kbps,
        &request.target_id,
        &request.mode,
    );

    let output_directory = request
        .output_directory
        .as_deref()
        .filter(|directory| Path::new(directory).is_dir())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            input
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        });

    let output = unique_output_path(&input, target.label, &output_directory);
    let filter = plan::build_filter(&upscale_plan, mode);
    let args = build_arguments(
        &input,
        &output,
        &filter,
        probe.has_audio(),
        audio_can_be_copied(probe.audio_codec.as_deref()),
    );

    let Some(ffmpeg) = media::ffmpeg_path() else {
        return Err("FFMPEG_MISSING".to_string());
    };

    let mut child = Command::new(&ffmpeg)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "FFMPEG_MISSING".to_string())?;

    let pid = child.id();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = slot.lock().map_err(|_| "STATE".to_string())?;
        *guard = Some(ActiveProcess {
            pid,
            cancel: cancel.clone(),
        });
    }

    let started = Instant::now();
    let duration = probe.duration;
    let app_progress = app.clone();

    let stdout = child.stdout.take().ok_or("FFMPEG_FAILED")?;
    let progress_thread = std::thread::spawn(move || {
        let mut lines = BufReader::new(stdout).lines();
        let mut last_percent = -1;
        while let Some(Ok(line)) = lines.next() {
            let Some(value) = line.strip_prefix("out_time_us=") else {
                continue;
            };
            let Ok(microseconds) = value.trim().parse::<i64>() else {
                continue;
            };
            if duration <= 0.0 {
                continue;
            }
            let percent =
                ((microseconds as f64 / 1_000_000.0 / duration) * 100.0).clamp(0.0, 99.0) as i32;
            if percent != last_percent {
                last_percent = percent;
                let _ = app_progress.emit(
                    "upscale-progress",
                    serde_json::json!({
                        "percent": percent,
                        "elapsed": started.elapsed().as_secs_f64()
                    }),
                );
            }
        }
    });

    let stderr = child.stderr.take().ok_or("FFMPEG_FAILED")?;
    let stderr_thread = std::thread::spawn(move || {
        let mut lines = BufReader::new(stderr).lines();
        let mut buffer = String::new();
        while let Some(Ok(line)) = lines.next() {
            if buffer.len() < 64_000 {
                buffer.push_str(&line);
                buffer.push('\n');
            }
        }
        buffer
    });

    let status = child.wait();
    let _ = progress_thread.join();
    let stderr_text = stderr_thread.join().unwrap_or_default();
    let was_cancelled = cancel.load(Ordering::SeqCst);

    if let Ok(mut guard) = slot.lock() {
        *guard = None;
    }

    if was_cancelled {
        let _ = std::fs::remove_file(&output);
        return Err("CANCELLED".to_string());
    }

    if !matches!(status, Ok(status) if status.success()) {
        let _ = std::fs::remove_file(&output);
        let detail = stderr_text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" ");
        return Err(format!(
            "FFMPEG_FAILED: {}",
            if detail.is_empty() {
                "FFmpeg failed."
            } else {
                &detail
            }
        ));
    }

    let _ = app.emit(
        "upscale-progress",
        serde_json::json!({
            "percent": 100,
            "elapsed": started.elapsed().as_secs_f64()
        }),
    );

    let output_bytes = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    let output_thumbnail = media::file_details(&output.to_string_lossy())
        .ok()
        .and_then(|details| details.thumbnail);

    Ok(UpscaleResult {
        output_path: output.to_string_lossy().into_owned(),
        output_name: output
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("video_escalado.mp4")
            .to_string(),
        output_bytes,
        input_bytes: input_metadata.len(),
        thumbnail: output_thumbnail,
        elapsed_seconds: started.elapsed().as_secs_f64(),
        plan: upscale_plan,
    })
}

fn emit_progress(app: &AppHandle, percent: i32, started: Instant) {
    emit_progress_detail(app, percent, started, None);
}

fn emit_progress_detail(app: &AppHandle, percent: i32, started: Instant, detail: Option<String>) {
    let _ = app.emit(
        "upscale-progress",
        serde_json::json!({
            "percent": percent.clamp(0, 100),
            "elapsed": started.elapsed().as_secs_f64(),
            "detail": detail
        }),
    );
}

/// Spawn a stage process and register its PID in the shared slot so the
/// existing cancel command kills whichever stage is currently running.
fn spawn_registered(
    slot: &ActiveSlot,
    command: &mut Command,
) -> Result<std::process::Child, String> {
    let child = command.spawn().map_err(|e| e.to_string())?;
    if let Ok(mut guard) = slot.lock() {
        if let Some(active) = guard.as_mut() {
            active.pid = child.id();
        }
    }
    Ok(child)
}

fn count_files(dir: &Path, extension: &str) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext.eq_ignore_ascii_case(extension))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

fn sorted_files(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .map(|ext| ext.eq_ignore_ascii_case(extension))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

fn log_tail(path: &Path, max_chars: usize) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let tail: Vec<&str> = lines.iter().rev().take(4).rev().cloned().collect();
    let joined = tail.join(" ");
    if joined.chars().count() > max_chars {
        joined.chars().rev().take(max_chars).collect::<String>().chars().rev().collect()
    } else {
        joined
    }
}

/// Real-ESRGAN pipeline: extract PNG frames → realesrgan-ncnn-vulkan (hosts
/// everything on the GPU, so all frames go in a single pass) → ffmpeg encode
/// to the selected target with the original audio muxed back.
#[allow(clippy::too_many_arguments)]
fn start_ai(
    app: &AppHandle,
    slot: &ActiveSlot,
    paths: &EnginePaths,
    request: &UpscaleRequest,
    input: &Path,
    input_metadata: &std::fs::Metadata,
    probe: &media::ProbeInfo,
    engine_id: &str,
) -> Result<UpscaleResult, String> {
    let model = vsr::def(paths, engine_id).ok_or_else(|| "MODEL_MISSING".to_string())?;
    let esrgan = vsr::find_realesrgan(paths).ok_or_else(|| "SD_MISSING".to_string())?;
    let models_dir = vsr::models_dir(paths).ok_or_else(|| "SD_MISSING".to_string())?;
    let ffmpeg = media::ffmpeg_path().ok_or_else(|| "FFMPEG_MISSING".to_string())?;

    let factor = model.factor;
    let ai_width = probe.width.saturating_mul(factor);
    let ai_height = probe.height.saturating_mul(factor);
    let target = plan::target_by_id(&request.target_id);
    let mode = Mode::parse(&request.mode);
    let upscale_plan = plan::compute_plan(
        ai_width,
        ai_height,
        probe.duration,
        probe.audio_kbps,
        &request.target_id,
        &request.mode,
    );

    let output_directory = request
        .output_directory
        .as_deref()
        .filter(|directory| Path::new(directory).is_dir())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            input
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        });
    let output = unique_output_path(input, target.label, &output_directory);

    let started = Instant::now();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = slot.lock().map_err(|_| "STATE".to_string())?;
        *guard = Some(ActiveProcess {
            pid: 0,
            cancel: cancel.clone(),
        });
    }

    let work = std::env::temp_dir().join(format!(
        "panto-studio-esr-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let frames_in = work.join("in");
    let frames_out = work.join("out");

    let result = run_ai_stages(
        app,
        slot,
        &cancel,
        started,
        EsrJob {
            ffmpeg: &ffmpeg,
            esrgan: &esrgan,
            models_dir: &models_dir,
            factor,
            input,
            output: &output,
            probe,
            upscale_plan: &upscale_plan,
            mode,
            frames_in: &frames_in,
            frames_out: &frames_out,
            work: &work,
        },
    );

    let _ = std::fs::remove_dir_all(&work);
    if let Ok(mut guard) = slot.lock() {
        *guard = None;
    }

    if cancel.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(&output);
        return Err("CANCELLED".to_string());
    }

    result?;

    emit_progress(app, 100, started);
    let output_bytes = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    let output_thumbnail = media::file_details(&output.to_string_lossy())
        .ok()
        .and_then(|details| details.thumbnail);

    Ok(UpscaleResult {
        output_path: output.to_string_lossy().into_owned(),
        output_name: output
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("video_escalado.mp4")
            .to_string(),
        output_bytes,
        input_bytes: input_metadata.len(),
        thumbnail: output_thumbnail,
        elapsed_seconds: started.elapsed().as_secs_f64(),
        plan: upscale_plan,
    })
}

struct EsrJob<'a> {
    ffmpeg: &'a str,
    esrgan: &'a Path,
    models_dir: &'a Path,
    factor: u32,
    input: &'a Path,
    output: &'a Path,
    probe: &'a media::ProbeInfo,
    upscale_plan: &'a Plan,
    mode: Mode,
    frames_in: &'a Path,
    frames_out: &'a Path,
    work: &'a Path,
}

#[allow(clippy::too_many_arguments)]
fn run_ai_stages(
    app: &AppHandle,
    slot: &ActiveSlot,
    cancel: &Arc<AtomicBool>,
    started: Instant,
    job: EsrJob,
) -> Result<(), String> {
    std::fs::create_dir_all(job.frames_in).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(job.frames_out).map_err(|e| e.to_string())?;
    let fps = if job.probe.fps > 0.0 { job.probe.fps } else { 30.0 };

    // Stage 1 (0–5%): extract frames as PNG — the bundled realesrgan
    // runner cannot decode JPEG.
    emit_progress(app, 1, started);
    let mut extract = Command::new(job.ffmpeg);
    extract
        .args(["-hide_banner", "-y", "-i"])
        .arg(job.input)
        .args(["-map", "0:v:0", "-an"])
        .arg(job.frames_in.join("%06d.png"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = spawn_registered(slot, &mut extract)?;
    let status = child.wait().map_err(|e| e.to_string())?;
    if cancel.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }
    if !status.success() {
        return Err("FFMPEG_FAILED: frame extraction failed.".to_string());
    }
    let input_frames = sorted_files(job.frames_in, "png");
    let total = input_frames.len();
    if total == 0 {
        return Err("FFMPEG_FAILED: no frames extracted.".to_string());
    }
    emit_progress(app, 5, started);

    // Stage 2 (5–90%): the compact Real-ESRGAN runner keeps its whole state
    // on the GPU, so every frame goes through in a single invocation.
    let log_path = job.work.join("esrgan.log");
    let log_file = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let log_file_err = log_file.try_clone().map_err(|e| e.to_string())?;
    let mut command = Command::new(job.esrgan);
    command
        .arg("-i")
        .arg(job.frames_in)
        .arg("-o")
        .arg(job.frames_out)
        .args(["-n", "realesr-animevideov3", "-s"])
        .arg(job.factor.to_string())
        .args(["-m"]).arg(job.models_dir)
        .args(["-f", "png"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_file_err));
    let mut child = spawn_registered(slot, &mut command)?;

    let mut saw = 0usize;
    let status = loop {
        if cancel.load(Ordering::SeqCst) {
            process::terminate(child.id());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                let produced = count_files(job.frames_out, "png");
                if produced > saw {
                    saw = produced;
                    let percent = 5 + (produced * 85 / total.max(1)) as i32;
                    emit_progress_detail(
                        app,
                        percent,
                        started,
                        Some(format!("frame {}/{}", produced.min(total), total)),
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    if cancel.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }
    if !status.success() {
        return Err(format!("SD_FAILED: {}", log_tail(&log_path, 400)));
    }

    // Stage 3 (90–100%): encode to the selected target and mux the audio.
    emit_progress(app, 90, started);
    let filter = plan::build_filter(job.upscale_plan, job.mode);
    let mut args: Vec<String> = vec![
        "-hide_banner".to_string(),
        "-y".to_string(),
        "-framerate".to_string(),
        format!("{fps:.3}"),
        "-i".to_string(),
        job.frames_out.join("%06d.png").to_string_lossy().into_owned(),
        "-i".to_string(),
        job.input.to_string_lossy().into_owned(),
        "-map".to_string(),
        "0:v:0".to_string(),
    ];
    if job.probe.has_audio() {
        args.push("-map".to_string());
        args.push("1:a?".to_string());
        if audio_can_be_copied(job.probe.audio_codec.as_deref()) {
            args.push("-c:a".to_string());
            args.push("copy".to_string());
        } else {
            args.push("-c:a".to_string());
            args.push("aac".to_string());
            args.push("-b:a".to_string());
            args.push("192k".to_string());
        }
    } else {
        args.push("-an".to_string());
    }
    args.extend([
        "-vf".to_string(),
        filter,
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "14".to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        "-shortest".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-nostats".to_string(),
        job.output.to_string_lossy().into_owned(),
    ]);

    let mut encode = Command::new(job.ffmpeg);
    encode
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn_registered(slot, &mut encode)?;
    let duration = job.probe.duration;
    let app_progress = app.clone();
    let stdout = child.stdout.take().ok_or("FFMPEG_FAILED")?;
    let progress_thread = std::thread::spawn(move || {
        let mut lines = BufReader::new(stdout).lines();
        while let Some(Ok(line)) = lines.next() {
            let Some(value) = line.strip_prefix("out_time_us=") else {
                continue;
            };
            let Ok(microseconds) = value.trim().parse::<i64>() else {
                continue;
            };
            if duration > 0.0 {
                let ratio = (microseconds as f64 / 1_000_000.0 / duration).clamp(0.0, 1.0);
                emit_progress(&app_progress, 90 + (ratio * 9.0) as i32, started);
            }
        }
    });
    let stderr = child.stderr.take().ok_or("FFMPEG_FAILED")?;
    let stderr_thread = std::thread::spawn(move || {
        let mut lines = BufReader::new(stderr).lines();
        let mut buffer = String::new();
        while let Some(Ok(line)) = lines.next() {
            if buffer.len() < 64_000 {
                buffer.push_str(&line);
                buffer.push('\n');
            }
        }
        buffer
    });
    let status = child.wait().map_err(|e| e.to_string())?;
    let _ = progress_thread.join();
    let stderr_text = stderr_thread.join().unwrap_or_default();
    if cancel.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }
    if !status.success() {
        let detail = stderr_text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" ");
        return Err(format!(
            "FFMPEG_FAILED: {}",
            if detail.is_empty() { "FFmpeg failed." } else { &detail }
        ));
    }
    Ok(())
}
