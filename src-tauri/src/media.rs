//! Media file helpers: extension classification, best-effort ffmpeg probing,
//! and thumbnail extraction shared by the three Panto Studio tabs.

use serde::Serialize;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

pub const AUDIO_EXTENSIONS: &[&str] = &[
    ".mp3", ".wav", ".m4a", ".aac", ".flac", ".ogg", ".opus", ".wma", ".aiff", ".aif", ".caf",
];
pub const VIDEO_EXTENSIONS: &[&str] = &[
    ".mov", ".mp4", ".m4v", ".avi", ".mkv", ".webm", ".mpg", ".mpeg", ".m2ts", ".ts",
];
pub const IMAGE_EXTENSIONS: &[&str] = &[
    ".jpg", ".jpeg", ".png", ".webp", ".bmp", ".tif", ".tiff",
];

pub fn media_type_for_file(file_path: &str) -> Option<&'static str> {
    let ext = Path::new(file_path)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Some("audio")
    } else if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        Some("video")
    } else if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        Some("image")
    } else {
        None
    }
}

/// Formats accepted by the Subtítulos tab.
pub fn is_supported(file_path: &str) -> bool {
    matches!(media_type_for_file(file_path), Some("audio") | Some("video"))
}

/// Formats accepted by the Convertir tab.
pub fn is_convertible(file_path: &str) -> bool {
    media_type_for_file(file_path).is_some()
}

pub fn is_video(file_path: &str) -> bool {
    media_type_for_file(file_path) == Some("video")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDetails {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub media_type: Option<String>,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub codec: String,
    pub audio_codec: Option<String>,
    pub audio_kbps: u32,
    pub has_audio: bool,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProbeInfo {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub fps: f64,
    pub codec: String,
    pub audio_codec: Option<String>,
    pub audio_kbps: u32,
}

impl ProbeInfo {
    pub fn has_audio(&self) -> bool {
        self.audio_codec.is_some()
    }
}

pub fn find_binary(name: &str) -> Option<String> {
    let executable = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };

    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            let candidate = directory.join(&executable);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
    }

    for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        let candidate = format!("{dir}/{executable}");
        if Path::new(&candidate).exists() {
            return Some(candidate);
        }
    }
    None
}

pub fn ffmpeg_path() -> Option<String> {
    find_binary("ffmpeg")
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

fn parse_duration(stderr: &str) -> f64 {
    let Some(pos) = stderr.find("Duration:") else {
        return 0.0;
    };
    let rest = stderr[pos + "Duration:".len()..].trim_start();
    let token: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ':' || *c == '.')
        .collect();
    parse_timestamp(&token)
}

/// Finds the first `NNNxNNN` token on a line.
fn find_dimensions(line: &str) -> Option<(u32, u32)> {
    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'x' {
            continue;
        }
        let mut start = index;
        while start > 0 && bytes[start - 1].is_ascii_digit() {
            start -= 1;
        }
        let mut end = index + 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        let left = &line[start..index];
        let right = &line[index + 1..end];
        if left.len() >= 2 && left.len() <= 5 && right.len() >= 2 && right.len() <= 5 {
            if let (Ok(width), Ok(height)) = (left.parse::<u32>(), right.parse::<u32>()) {
                if width > 0 && height > 0 {
                    return Some((width, height));
                }
            }
        }
    }
    None
}

fn parse_fps(line: &str) -> f64 {
    let Some(pos) = line.find(" fps") else {
        return 0.0;
    };
    let before = &line[..pos];
    let number: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    number.parse().unwrap_or(0.0)
}

fn parse_audio_kbps(line: &str) -> u32 {
    let Some(pos) = line.find(" kb/s") else {
        return 0;
    };
    let before = &line[..pos];
    let number: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    number.parse().unwrap_or(0)
}

fn token_after(line: &str, marker: &str) -> String {
    line.split(marker)
        .nth(1)
        .map(|rest| {
            rest.trim()
                .split(|c: char| c == ' ' || c == ',' || c == '(' || c == ')')
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_default()
}

fn parse_rotation(line: &str) -> Option<f64> {
    for marker in ["rotation of", "rotate"] {
        if let Some(pos) = line.find(marker) {
            let rest = &line[pos + marker.len()..];
            let rest = rest.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
            let number: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                .collect();
            if let Ok(value) = number.parse::<f64>() {
                return Some(value);
            }
        }
    }
    None
}

/// Runs `ffmpeg -i` and parses its stderr. Works for audio, video and images.
pub fn probe_media(file_path: &str) -> ProbeInfo {
    let Some(ffmpeg) = ffmpeg_path() else {
        return ProbeInfo::default();
    };
    let Ok(mut child) = Command::new(&ffmpeg)
        .args(["-hide_banner", "-i", file_path])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return ProbeInfo::default();
    };

    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }
    let _ = child.wait();

    let duration = parse_duration(&stderr);
    let mut width = 0;
    let mut height = 0;
    let mut fps = 0.0;
    let mut codec = String::new();
    let mut audio_codec: Option<String> = None;
    let mut audio_kbps = 0;
    let mut rotation = 0.0_f64;
    let mut found_video = false;

    for line in stderr.lines() {
        let trimmed = line.trim();

        if !found_video && trimmed.contains("Video:") && !trimmed.contains("attached pic") {
            if let Some((w, h)) = find_dimensions(trimmed) {
                width = w;
                height = h;
                found_video = true;
            }
            let value = parse_fps(trimmed);
            if value > 0.0 {
                fps = value;
            }
            codec = token_after(trimmed, "Video:");
        }

        if audio_codec.is_none() && trimmed.contains("Audio:") {
            let value = token_after(trimmed, "Audio:");
            if !value.is_empty() {
                audio_codec = Some(value);
            }
            audio_kbps = parse_audio_kbps(trimmed);
        }

        if rotation == 0.0 {
            if let Some(value) = parse_rotation(trimmed) {
                rotation = value;
            }
        }
    }

    if audio_codec.is_some() && audio_kbps == 0 {
        audio_kbps = 192;
    }

    if (89.5..=90.5).contains(&rotation.rem_euclid(180.0)) {
        std::mem::swap(&mut width, &mut height);
    }

    ProbeInfo {
        width,
        height,
        duration,
        fps,
        codec,
        audio_codec,
        audio_kbps,
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let combined = (b0 << 16) | (b1 << 8) | b2;
        output.push(BASE64[((combined >> 18) & 63) as usize] as char);
        output.push(BASE64[((combined >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            output.push(BASE64[((combined >> 6) & 63) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(BASE64[(combined & 63) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn thumbnail_for(file_path: &str, media_type: Option<&str>, duration: f64) -> Option<String> {
    let ffmpeg = ffmpeg_path()?;
    let mut args: Vec<String> = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
    ];
    if media_type != Some("image") && duration > 3.0 {
        args.push("-ss".to_string());
        args.push(format!("{:.3}", (duration * 0.12).min(3.0)));
    }
    args.extend([
        "-i".to_string(),
        file_path.to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        "-vf".to_string(),
        "scale=480:-2:flags=lanczos".to_string(),
        "-f".to_string(),
        "image2pipe".to_string(),
        "-vcodec".to_string(),
        "png".to_string(),
        "pipe:1".to_string(),
    ]);

    let output = Command::new(&ffmpeg)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() || output.stdout.is_empty() || output.stdout.len() > 12 * 1024 * 1024
    {
        return None;
    }
    Some(format!(
        "data:image/png;base64,{}",
        base64_encode(&output.stdout)
    ))
}

pub fn file_details(file_path: &str) -> Result<FileDetails, String> {
    let metadata = std::fs::metadata(file_path).map_err(|e| e.to_string())?;
    let name = Path::new(file_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let media_type = media_type_for_file(file_path);
    let info = probe_media(file_path);
    let thumbnail = thumbnail_for(file_path, media_type, info.duration);

    Ok(FileDetails {
        path: file_path.to_string(),
        name,
        size: metadata.len(),
        media_type: media_type.map(|s| s.to_string()),
        duration: info.duration,
        width: info.width,
        height: info.height,
        fps: info.fps,
        codec: info.codec.clone(),
        audio_codec: info.audio_codec.clone(),
        audio_kbps: info.audio_kbps,
        has_audio: info.has_audio(),
        thumbnail,
    })
}
