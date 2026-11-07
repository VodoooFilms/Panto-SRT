//! Parakeet TDT 0.6B v3 engine via the bundled `nemo-speech` CLI
//! (NeMo-Speech.cpp, GGUF Q8). Emits JSON with word-level timestamps,
//! which map directly onto our `Word` stream.

use crate::grouping::Word;
use crate::whisper::{EnginePaths, TempAudio, TranscribeResult, Utterance};
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub fn find_engine(paths: &EnginePaths) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        "nemo-speech.exe"
    } else {
        "nemo-speech"
    };
    let mut candidates = Vec::new();
    if let Some(res) = &paths.resource_dir {
        candidates.push(res.join("nemo").join("bin").join(exe));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = manifest.parent() {
        candidates.push(root.join("build").join("nemo").join("bin").join(exe));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin/nemo-speech"));
    candidates.push(PathBuf::from("/usr/local/bin/nemo-speech"));
    candidates.into_iter().find(|c| c.exists())
}

/// nemo-speech only ingests WAV, so always normalize to 16 kHz mono WAV.
fn ready_wav(file_path: &str) -> TempAudio {
    if file_path.to_lowercase().ends_with(".wav") {
        return TempAudio::borrowed(file_path);
    }
    let Some(ffmpeg) = crate::media::find_binary("ffmpeg") else {
        return TempAudio::borrowed(file_path);
    };
    let output = std::env::temp_dir().join(format!(
        "panto-srt-pk-{}-{}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        std::process::id()
    ));
    let ok = Command::new(&ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            file_path,
            "-ac",
            "1",
            "-ar",
            "16000",
        ])
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok && output.exists() {
        TempAudio::owned(output)
    } else {
        TempAudio::borrowed(file_path)
    }
}

pub fn transcribe(
    paths: &EnginePaths,
    file_path: &str,
    model_path: &std::path::Path,
    _language: &str,
    _duration: f64,
    cancelled: &AtomicBool,
    child_pid: &Arc<Mutex<Option<u32>>>,
    on_progress: &dyn Fn(i32),
) -> Result<TranscribeResult, String> {
    let engine = find_engine(paths).ok_or_else(|| "engine-missing".to_string())?;
    if !model_path.exists() {
        return Err("model-missing:parakeet".to_string());
    }
    let audio = ready_wav(file_path);
    let output = std::env::temp_dir().join(format!(
        "panto-srt-pk-{}-{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        std::process::id()
    ));

    on_progress(5);
    let mut child = Command::new(&engine)
        .arg("transcribe")
        .arg(audio.path())
        .args(["--model"])
        .arg(model_path)
        .args(["--format", "json", "-o"])
        .arg(&output)
        .arg("--force")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    *child_pid.lock().unwrap() = Some(child.id());

    let status = loop {
        if cancelled.load(Ordering::SeqCst) {
            let _ = child.kill();
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(120)),
            Err(e) => return Err(e.to_string()),
        }
    };
    *child_pid.lock().unwrap() = None;

    if cancelled.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(&output);
        return Err("cancelled".to_string());
    }
    if !status.success() {
        let _ = std::fs::remove_file(&output);
        return Err(format!("El motor terminó con código {:?}.", status.code()));
    }
    let raw = std::fs::read_to_string(&output).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&output);
    let result: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    on_progress(95);

    let words: Vec<Word> = result
        .get("words")
        .and_then(|w| w.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let text = item.get("word")?.as_str()?.trim().to_string();
                    if text.is_empty() {
                        return None;
                    }
                    Some(Word {
                        text,
                        start: item.get("start").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        end: item.get("end").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let text = result
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let duration = result
        .get("duration")
        .and_then(|d| d.as_f64())
        .unwrap_or_else(|| words.iter().map(|w| w.end).fold(0.0, f64::max));

    on_progress(100);
    Ok(TranscribeResult {
        utterances: if text.is_empty() {
            Vec::new()
        } else {
            vec![Utterance {
                start: 0.0,
                end: duration,
                text,
            }]
        },
        words,
        duration,
        detected_language: "auto".to_string(),
    })
}
