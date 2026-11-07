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
use crate::process::{ActiveProcess, ActiveSlot};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleRequest {
    pub input_path: String,
    pub target_id: String,
    pub mode: String,
    pub output_directory: Option<String>,
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
