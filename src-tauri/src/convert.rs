//! Pantoraya's conversion profiles and ffmpeg pipeline, ported to Rust.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::media;
use crate::process::{self, ActiveSlot, ActiveProcess};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequest {
    pub input_path: String,
    pub converter: String,
    pub profile: String,
    pub subtitle_path: Option<String>,
    pub output_directory: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionResult {
    pub output_path: String,
    pub output_bytes: u64,
    pub input_bytes: u64,
    pub converter_id: String,
    pub profile_id: String,
}

pub fn output_extension(converter: &str) -> Option<&'static str> {
    match converter {
        "mp4" => Some(".mp4"),
        "mp3" => Some(".mp3"),
        "jpg" => Some(".jpg"),
        _ => None,
    }
}

fn profile_spec(converter: &str, profile: &str, lang: &str) -> Result<(Vec<String>, &'static str), String> {
    let spanish = lang != "en";
    let spec = match (converter, profile) {
        ("mp4", "quality") => (
            vec![
                "-c:v", "libx264", "-preset", "medium", "-crf", "20", "-c:a", "aac", "-b:a",
                "192k",
            ],
            "",
        ),
        ("mp4", "light") => (
            vec![
                "-c:v",
                "libx264",
                "-preset",
                "fast",
                "-crf",
                "27",
                "-vf",
                "scale=w=min(1280\\,iw):h=min(720\\,ih):force_original_aspect_ratio=decrease:force_divisible_by=2",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
            ],
            if spanish { "_liviano" } else { "_light" },
        ),
        ("mp3", "high") => (
            vec!["-c:a", "libmp3lame", "-b:a", "320k"],
            "_mp3",
        ),
        ("mp3", "light") => (
            vec!["-c:a", "libmp3lame", "-b:a", "128k"],
            if spanish { "_mp3-liviano" } else { "_mp3-light" },
        ),
        ("jpg", "high") => (
            vec![
                "-filter_complex",
                "[0:v]format=rgba[fg];color=c=white:s=16x16[bg];[bg][fg]scale2ref[bg][fg];[bg][fg]overlay=shortest=1,format=yuvj420p[v]",
                "-map",
                "[v]",
                "-c:v",
                "mjpeg",
                "-q:v",
                "2",
            ],
            if spanish { "_comprimida" } else { "_compressed" },
        ),
        ("jpg", "light") => (
            vec![
                "-filter_complex",
                "[0:v]scale=w=min(1280\\,iw):h=min(1280\\,ih):force_original_aspect_ratio=decrease,format=rgba[fg];color=c=white:s=16x16[bg];[bg][fg]scale2ref[bg][fg];[bg][fg]overlay=shortest=1,format=yuvj420p[v]",
                "-map",
                "[v]",
                "-c:v",
                "mjpeg",
                "-q:v",
                "7",
            ],
            if spanish { "_liviana" } else { "_light" },
        ),
        _ => return Err("INVALID_PROFILE".to_string()),
    };
    Ok((spec.0.into_iter().map(str::to_string).collect(), spec.1))
}

fn output_args(converter: &str) -> &'static [&'static str] {
    match converter {
        "mp4" => &["-pix_fmt", "yuv420p", "-movflags", "+faststart"],
        "mp3" => &["-map_metadata", "0", "-id3v2_version", "3"],
        "jpg" => &["-frames:v", "1", "-update", "1", "-map_metadata", "0"],
        _ => &[],
    }
}

fn stream_args(converter: &str, input: &str, subtitle: Option<&str>) -> Vec<String> {
    if converter == "mp3" {
        if media::is_video(input) {
            return vec!["-vn".to_string()];
        }
        return vec![
            "-map".to_string(),
            "0:a:0".to_string(),
            "-map".to_string(),
            "0:v?".to_string(),
            "-c:v".to_string(),
            "copy".to_string(),
            "-disposition:v".to_string(),
            "attached_pic".to_string(),
        ];
    }

    if converter == "mp4" && subtitle.is_some() {
        return vec![
            "-map".to_string(),
            "0:v:0".to_string(),
            "-map".to_string(),
            "0:a?".to_string(),
            "-map".to_string(),
            "1:0".to_string(),
            "-c:s".to_string(),
            "mov_text".to_string(),
            "-metadata:s:s:0".to_string(),
            "language=und".to_string(),
        ];
    }

    Vec::new()
}

fn unique_output_path(
    input: &str,
    converter: &str,
    suffix: &str,
    directory: Option<&Path>,
) -> PathBuf {
    let source = Path::new(input);
    let parent = directory
        .map(Path::to_path_buf)
        .or_else(|| source.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let name = source
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let input_ext = source
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let output_ext = output_extension(converter).unwrap_or(".mp4");
    let suffix = if input_ext == output_ext { "" } else { suffix };

    let mut candidate = parent.join(format!("{name}{suffix}{output_ext}"));
    let mut count = 2;
    while candidate.exists() {
        candidate = parent.join(format!("{name}{suffix}-{count}{output_ext}"));
        count += 1;
    }
    candidate
}

fn parse_timestamp(value: &str) -> f64 {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 3 {
        return 0.0;
    }
    let h: f64 = parts[0].parse().unwrap_or(0.0);
    let m: f64 = parts[1].parse().unwrap_or(0.0);
    let s: f64 = parts[2].parse().unwrap_or(0.0);
    h * 3600.0 + m * 60.0 + s
}

fn timestamp_from(line: &str, marker: &str) -> Option<String> {
    let pos = line.find(marker)?;
    let rest = &line[pos + marker.len()..];
    let token: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ':' || *c == '.')
        .collect();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// Runs one conversion. This is blocking: call it from `spawn_blocking`.
pub fn convert(
    app: &AppHandle,
    slot: &ActiveSlot,
    request: ConvertRequest,
    lang: &str,
) -> Result<ConversionResult, String> {
    if !media::is_convertible(&request.input_path) || !Path::new(&request.input_path).exists() {
        return Err("MISSING".to_string());
    }
    let converter = request.converter.as_str();
    if output_extension(converter).is_none() {
        return Err("INVALID_PROFILE".to_string());
    }
    let (profile_args, suffix) = profile_spec(converter, &request.profile, lang)?;

    let subtitle = request.subtitle_path.as_deref().filter(|path| {
        converter == "mp4"
            && Path::new(path)
                .extension()
                .map(|e| e.eq_ignore_ascii_case("srt"))
                .unwrap_or(false)
            && Path::new(path).exists()
    });

    {
        let guard = slot.lock().map_err(|_| "STATE".to_string())?;
        if guard.is_some() {
            return Err("BUSY".to_string());
        }
    }

    let output_directory = request
        .output_directory
        .as_deref()
        .filter(|directory| Path::new(directory).is_dir())
        .map(Path::new);
    let output = unique_output_path(&request.input_path, converter, suffix, output_directory);
    let input_bytes = std::fs::metadata(&request.input_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let Some(ffmpeg) = media::ffmpeg_path() else {
        return Err("FFMPEG_MISSING".to_string());
    };

    let mut args: Vec<String> = vec![
        "-hide_banner".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        request.input_path.clone(),
    ];
    if let Some(subtitle) = subtitle {
        args.push("-i".to_string());
        args.push(subtitle.to_string());
    }
    args.extend(stream_args(converter, &request.input_path, subtitle));
    args.extend(profile_args);
    args.extend(output_args(converter).iter().map(|s| s.to_string()));
    args.push(output.to_string_lossy().into_owned());

    let mut child = Command::new(&ffmpeg)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "FFMPEG_MISSING".to_string())?;

    let pid = child.id();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let mut guard = slot.lock().map_err(|_| "STATE".to_string())?;
        *guard = Some(ActiveProcess {
            pid,
            cancel: cancel.clone(),
        });
    }

    let _ = app.emit(
        "convert-status",
        serde_json::json!({
            "status": "starting",
            "message": if lang == "en" { "Converting…" } else { "Convirtiendo…" }
        }),
    );

    let stderr = child.stderr.take().ok_or("FFMPEG_FAILED")?;
    let mut reader = BufReader::new(stderr);
    let mut duration = 0.0_f64;
    let mut log = String::new();
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        match reader.read_until(b'\r', &mut buffer) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        let text = String::from_utf8_lossy(&buffer);
        for line in text.split(['\n', '\r']) {
            if line.trim().is_empty() {
                continue;
            }
            if duration == 0.0 {
                if let Some(value) = timestamp_from(line, "Duration:") {
                    duration = parse_timestamp(&value);
                }
            }
            if let Some(stamp) = timestamp_from(line, "time=") {
                if duration > 0.0 {
                    let current = parse_timestamp(&stamp);
                    let percent = ((current / duration) * 100.0).clamp(0.0, 99.0) as i32;
                    let _ = app.emit(
                        "convert-progress",
                        serde_json::json!({ "percent": percent, "timemark": stamp }),
                    );
                }
            }
            if log.len() < 16_000 {
                log.push_str(line);
                log.push('\n');
            }
        }
    }

    let status = child.wait();
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
        let detail = log
            .lines()
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(3)
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
        "convert-progress",
        serde_json::json!({ "percent": 100, "timemark": "" }),
    );
    process::play_completion_sound();

    let output_bytes = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    Ok(ConversionResult {
        output_path: output.to_string_lossy().into_owned(),
        output_bytes,
        input_bytes,
        converter_id: request.converter,
        profile_id: request.profile,
    })
}

/// Validate that a path is an SRT file that exists.
pub fn subtitle_file(path: &str) -> Option<PathBuf> {
    let candidate = Path::new(path);
    let is_srt = candidate
        .extension()
        .map(|e| e.eq_ignore_ascii_case("srt"))
        .unwrap_or(false);
    if is_srt && candidate.is_file() {
        Some(candidate.to_path_buf())
    } else {
        None
    }
}

