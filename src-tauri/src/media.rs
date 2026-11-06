//! Media file helpers: supported extensions + best-effort duration probing.

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

pub fn media_type_for_file(file_path: &str) -> Option<&'static str> {
    let ext = Path::new(file_path)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Some("audio")
    } else if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        Some("video")
    } else {
        None
    }
}

pub fn is_supported(file_path: &str) -> bool {
    media_type_for_file(file_path).is_some()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDetails {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub media_type: Option<String>,
    pub duration: f64,
}

pub fn find_binary(name: &str) -> Option<String> {
    for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        let candidate = format!("{dir}/{name}");
        if Path::new(&candidate).exists() {
            return Some(candidate);
        }
    }
    // Fall back to PATH lookup.
    if Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success() || true)
        .unwrap_or(false)
    {
        return Some(name.to_string());
    }
    None
}

fn parse_duration(stderr: &str) -> f64 {
    // Looks for "Duration: 00:01:23.45"
    let Some(pos) = stderr.find("Duration:") else {
        return 0.0;
    };
    let rest = stderr[pos + "Duration:".len()..].trim_start();
    let token: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ':' || *c == '.')
        .collect();
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 3 {
        return 0.0;
    }
    let h: f64 = parts[0].parse().unwrap_or(0.0);
    let m: f64 = parts[1].parse().unwrap_or(0.0);
    let s: f64 = parts[2].parse().unwrap_or(0.0);
    h * 3600.0 + m * 60.0 + s
}

pub fn probe_duration(file_path: &str) -> f64 {
    let Some(ffmpeg) = find_binary("ffmpeg") else {
        return 0.0;
    };
    let Ok(mut child) = Command::new(&ffmpeg)
        .args(["-hide_banner", "-i", file_path])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return 0.0;
    };
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }
    let _ = child.wait();
    parse_duration(&stderr)
}

pub fn file_details(file_path: &str) -> Result<FileDetails, String> {
    let metadata = std::fs::metadata(file_path).map_err(|e| e.to_string())?;
    let name = Path::new(file_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(FileDetails {
        path: file_path.to_string(),
        name,
        size: metadata.len(),
        media_type: media_type_for_file(file_path).map(|s| s.to_string()),
        duration: probe_duration(file_path),
    })
}
