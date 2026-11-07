//! whisper.cpp engine wrapper: spawn `whisper-cli`, parse its full JSON
//! (`-ojf`) with word-level timestamps, emit progress while running.
//! GGML models are downloaded on demand into the app data folder.

use crate::grouping::Word;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub const MODELS: &[(&str, &str)] = &[
    ("small", "ggml-small.bin"),
    ("medium", "ggml-medium.bin"),
    ("turbo", "ggml-large-v3-turbo.bin"),
];

const MODEL_HOST: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Published download sizes, used to measure download progress.
pub fn model_size(model_id: &str) -> Option<u64> {
    match model_id {
        "small" => Some(487_601_967),
        "turbo" => Some(1_624_555_275),
        _ => None,
    }
}

/// SHA-256 of the published GGML files, used to reject corrupt or tampered
/// downloads before they are handed to the native engine.
pub fn model_sha256(model_id: &str) -> Option<&'static str> {
    match model_id {
        "small" => Some("1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b"),
        "turbo" => Some("1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69"),
        _ => None,
    }
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn model_file(model_id: &str) -> Option<&'static str> {
    MODELS
        .iter()
        .find(|(id, _)| *id == model_id)
        .map(|(_, f)| *f)
}

pub struct EnginePaths {
    pub resource_dir: Option<PathBuf>,
    pub app_data_dir: PathBuf,
}

fn engine_candidates(paths: &EnginePaths) -> Vec<PathBuf> {
    let exe = if cfg!(windows) {
        "whisper-cli.exe"
    } else {
        "whisper-cli"
    };
    let mut candidates = Vec::new();
    if let Some(res) = &paths.resource_dir {
        candidates.push(res.join("whisper").join(exe));
    }
    // Dev layout: <repo>/build/whisper/whisper-cli (src-tauri is one level down).
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = manifest.parent() {
        candidates.push(root.join("build").join("whisper").join(exe));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin/whisper-cli"));
    candidates.push(PathBuf::from("/usr/local/bin/whisper-cli"));
    candidates
}

pub fn find_engine(paths: &EnginePaths) -> Option<PathBuf> {
    engine_candidates(paths).into_iter().find(|c| c.exists())
}

pub fn models_dir(paths: &EnginePaths) -> PathBuf {
    let dir = paths.app_data_dir.join("models");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn find_model(paths: &EnginePaths, model_id: &str) -> Option<PathBuf> {
    let file = model_file(model_id)?;
    let mut candidates = vec![models_dir(paths).join(file)];
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = manifest.parent() {
        candidates.insert(0, root.join("build").join("whisper").join(file));
    }
    if let Some(res) = &paths.resource_dir {
        candidates.push(res.join("whisper").join(file));
    }
    candidates.into_iter().find(|c| c.exists())
}

/// Download a model with the system `curl`, reporting percent progress.
#[allow(dead_code)]
pub fn download_model(
    paths: &EnginePaths,
    model_id: &str,
    on_progress: &dyn Fn(i32),
) -> Result<PathBuf, String> {
    download_model_cancellable(paths, model_id, on_progress, None, None)
}

/// Cancellable variant used by the transcription flow, so pressing Cancel
/// during the automatic model download actually stops it.
pub fn download_model_cancellable(
    paths: &EnginePaths,
    model_id: &str,
    on_progress: &dyn Fn(i32),
    child_pid: Option<&Arc<Mutex<Option<u32>>>>,
    cancelled: Option<&AtomicBool>,
) -> Result<PathBuf, String> {
    if let Some(existing) = find_model(paths, model_id) {
        return Ok(existing);
    }
    let file = model_file(model_id).ok_or_else(|| format!("Modelo desconocido: {model_id}"))?;
    let destination = models_dir(paths).join(file);
    let url = format!("{MODEL_HOST}/{file}?download=true");
    download_url_cancellable(
        &url,
        &destination,
        model_size(model_id),
        model_sha256(model_id),
        on_progress,
        child_pid,
        cancelled,
    )?;
    Ok(destination)
}

/// Download any URL to `destination` (via a `.part` file) with the system
/// `curl`. Progress is measured from the `.part` file size against the
/// known total (curl emits no usable progress when its output is piped).
#[allow(dead_code)]
pub fn download_url(
    url: &str,
    destination: &Path,
    total: Option<u64>,
    on_progress: &dyn Fn(i32),
) -> Result<(), String> {
    download_url_cancellable(url, destination, total, None, on_progress, None, None)
}

/// Cancellable variant: registers the curl PID so another command can kill
/// it; when `cancelled` is set the partial file is removed and the call
/// returns `Err("cancelled")`. When `expected_sha256` is present the file is
/// verified before being moved into place.
pub fn download_url_cancellable(
    url: &str,
    destination: &Path,
    total: Option<u64>,
    expected_sha256: Option<&str>,
    on_progress: &dyn Fn(i32),
    child_pid: Option<&Arc<Mutex<Option<u32>>>>,
    cancelled: Option<&AtomicBool>,
) -> Result<(), String> {
    let tmp = destination.with_extension("bin.part");
    let _ = std::fs::remove_file(&tmp);

    let mut child = Command::new("curl")
        .args(["-L", "--fail", "-sS", "-o"])
        .arg(&tmp)
        .arg(url)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("curl no disponible: {e}"))?;
    if let Some(slot) = child_pid {
        *slot.lock().unwrap() = Some(child.id());
    }

    let mut last = -1i32;
    let status = loop {
        if cancelled
            .map(|flag| flag.load(Ordering::SeqCst))
            .unwrap_or(false)
        {
            let _ = child.kill();
        }
        if let Some(total) = total.filter(|t| *t > 0) {
            let downloaded = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
            let percent = ((downloaded * 100) / total).min(99) as i32;
            if percent > last {
                last = percent;
                on_progress(percent);
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(120)),
            Err(e) => return Err(e.to_string()),
        }
    };
    if let Some(slot) = child_pid {
        *slot.lock().unwrap() = None;
    }
    if cancelled
        .map(|flag| flag.load(Ordering::SeqCst))
        .unwrap_or(false)
    {
        let _ = std::fs::remove_file(&tmp);
        return Err("cancelled".to_string());
    }
    if !status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "La descarga terminó con código {:?}",
            status.code()
        ));
    }
    if let (Some(expected), Some(total)) = (
        expected_sha256.filter(|s| !s.is_empty()),
        total.filter(|t| *t > 0),
    ) {
        let downloaded = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
        if downloaded != total {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!(
                "La descarga está incompleta ({downloaded} de {total} bytes)."
            ));
        }
        let actual = sha256_file(&tmp)?;
        if !actual.eq_ignore_ascii_case(expected) {
            let _ = std::fs::remove_file(&tmp);
            return Err("La descarga no superó la verificación SHA-256.".to_string());
        }
    }
    std::fs::rename(&tmp, destination).map_err(|e| e.to_string())?;
    on_progress(100);
    Ok(())
}

/// Parse "00:01:05,500" / "00:01:05.500" or a number of seconds.
pub fn parse_timestamp(value: &Value) -> f64 {
    if let Some(n) = value.as_f64() {
        return (n * 1000.0).round() / 1000.0;
    }
    let Some(text) = value.as_str() else {
        return 0.0;
    };
    let normalized = text.replace(',', ".");
    let parts: Vec<&str> = normalized.split(':').collect();
    if parts.len() != 3 {
        return 0.0;
    }
    let h: f64 = parts[0].parse().unwrap_or(0.0);
    let m: f64 = parts[1].parse().unwrap_or(0.0);
    let s: f64 = parts[2].parse().unwrap_or(0.0);
    ((h * 3600.0 + m * 60.0 + s) * 1000.0).round() / 1000.0
}

fn item_start(item: &Value) -> f64 {
    if let Some(from) = item.pointer("/timestamps/from") {
        return parse_timestamp(from);
    }
    item.get("start").map(parse_timestamp).unwrap_or(0.0)
}

fn item_end(item: &Value) -> f64 {
    if let Some(to) = item.pointer("/timestamps/to") {
        return parse_timestamp(to);
    }
    item.get("end").map(parse_timestamp).unwrap_or(0.0)
}

fn is_special_token(text: &str) -> bool {
    text.starts_with("[_") && text.ends_with(']')
}

/// Whisper marks a new word with a leading space; `-sow` may split a word
/// across several tokens, so tokens without a leading space merge onto it.
fn tokens_to_words(tokens: &[Value]) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    for token in tokens {
        let text = token.get("text").and_then(|t| t.as_str()).unwrap_or("");
        if is_special_token(text) {
            continue;
        }
        if text.starts_with(|c: char| c.is_whitespace()) {
            words.push(Word {
                text: text.trim().to_string(),
                start: item_start(token),
                end: item_end(token),
            });
        } else if let Some(current) = words.last_mut() {
            current.text.push_str(text.trim());
            current.end = item_end(token);
        }
    }
    words
}

fn transcription_array(result: &Value) -> Vec<Value> {
    if let Some(items) = result.get("transcription").and_then(|v| v.as_array()) {
        return items.clone();
    }
    if let Some(items) = result.get("segments").and_then(|v| v.as_array()) {
        return items.clone();
    }
    Vec::new()
}

pub fn extract_words(result: &Value) -> Vec<Word> {
    let mut words = Vec::new();
    for item in transcription_array(result) {
        let tokens = item.get("tokens").and_then(|t| t.as_array());
        let has_token_timestamps = tokens
            .map(|tokens| {
                tokens.iter().any(|t| {
                    t.pointer("/timestamps/from").is_some() || t.pointer("/timestamps/to").is_some()
                })
            })
            .unwrap_or(false);
        if let (Some(tokens), true) = (tokens, has_token_timestamps) {
            words.extend(tokens_to_words(tokens));
            continue;
        }
        let text = item
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim();
        if !text.is_empty() {
            words.push(Word {
                text: text.to_string(),
                start: item_start(&item),
                end: item_end(&item),
            });
        }
    }
    words
}

#[derive(Debug, Clone, Serialize)]
pub struct Utterance {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

pub fn extract_utterances(result: &Value) -> Vec<Utterance> {
    transcription_array(result)
        .iter()
        .filter(|item| {
            !item
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .trim()
                .is_empty()
        })
        .map(|item| Utterance {
            start: item_start(item),
            end: item_end(item),
            text: item
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .trim()
                .to_string(),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeResult {
    pub words: Vec<Word>,
    pub utterances: Vec<Utterance>,
    pub duration: f64,
    pub detected_language: String,
}

/// Audio input for an engine call. When we had to create a temporary WAV, the
/// file is removed automatically as soon as the value goes out of scope.
pub struct TempAudio {
    path: String,
    owned: Option<PathBuf>,
}

impl TempAudio {
    pub fn borrowed(file_path: &str) -> Self {
        Self {
            path: file_path.to_string(),
            owned: None,
        }
    }

    pub fn owned(path: PathBuf) -> Self {
        Self {
            path: path.to_string_lossy().into_owned(),
            owned: Some(path),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Drop for TempAudio {
    fn drop(&mut self) {
        if let Some(path) = &self.owned {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Convert non-WAV/MP3/FLAC inputs to 16 kHz mono WAV with ffmpeg when present.
pub fn ready_audio(file_path: &str) -> TempAudio {
    let ext = Path::new(file_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if matches!(ext.as_str(), "wav" | "mp3" | "flac") {
        return TempAudio::borrowed(file_path);
    }
    let Some(ffmpeg) = crate::media::find_binary("ffmpeg") else {
        return TempAudio::borrowed(file_path);
    };
    let output = std::env::temp_dir().join(format!(
        "panto-srt-{}-{}.wav",
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
        TempAudio {
            path: output.to_string_lossy().into_owned(),
            owned: Some(output),
        }
    } else {
        TempAudio::borrowed(file_path)
    }
}

fn parse_progress_chunk(chunk: &str, duration: f64, last: &mut i32, on_progress: &dyn Fn(i32)) {
    // Window lines like "[00:00:01,000 --> 00:00:02,000]"
    if duration > 0.0 {
        for part in chunk.split("-->") {
            if let Some(end_part) = part.split(']').next() {
                let candidate = end_part.trim();
                let secs = parse_timestamp(&Value::String(candidate.to_string()));
                if secs > 0.0 {
                    let percent = ((secs / duration) * 100.0).round() as i32;
                    let percent = percent.min(99);
                    if percent > *last {
                        *last = percent;
                        on_progress(percent);
                    }
                }
            }
        }
    }
    // Bare percents like "progress = 42%" or "42%"
    for token in chunk.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '%')) {
        if let Some(num) = token.strip_suffix('%') {
            if let Ok(raw) = num.parse::<f64>() {
                if raw <= 100.0 {
                    let percent = (raw.round() as i32).min(99);
                    if percent > *last {
                        *last = percent;
                        on_progress(percent);
                    }
                }
            }
        }
    }
}

pub fn transcribe(
    paths: &EnginePaths,
    file_path: &str,
    model_id: &str,
    language: &str,
    duration: f64,
    cancelled: &AtomicBool,
    child_pid: &Arc<Mutex<Option<u32>>>,
    on_progress: &dyn Fn(i32),
) -> Result<TranscribeResult, String> {
    let engine = find_engine(paths).ok_or_else(|| "engine-missing".to_string())?;
    let model = find_model(paths, model_id).ok_or_else(|| format!("model-missing:{model_id}"))?;
    let language = if matches!(language, "es" | "en") {
        language
    } else {
        "auto"
    };

    let audio_file = ready_audio(file_path);
    let output_prefix = std::env::temp_dir().join(format!(
        "panto-srt-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        std::process::id()
    ));
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .max(2);

    let mut child = Command::new(&engine)
        .args([
            "-m",
            &model.to_string_lossy(),
            "-f",
            audio_file.path(),
            "-of",
            &output_prefix.to_string_lossy(),
            "-ojf",
            "-sow",
            "-l",
            language,
            "-t",
            &threads.to_string(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    *child_pid.lock().unwrap() = Some(child.id());

    let stderr_text = Arc::new(Mutex::new(String::new()));
    let mut handles = Vec::new();
    // Progress is reported on both stdout and stderr depending on the build.
    let pipes: Vec<Option<Box<dyn std::io::Read + Send>>> = vec![
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
    ];
    for pipe in pipes {
        let stderr_text = Arc::clone(&stderr_text);
        handles.push(std::thread::spawn(move || {
            if let Some(pipe) = pipe {
                let reader = BufReader::new(pipe);
                let mut last = -1i32;
                // Progress callbacks can't cross threads; collect raw lines and
                // parse here with a thread-local sink via channel substitute:
                // we parse and forward through a static-free approach below.
                let _ = &mut last;
                for line in reader.lines().map_while(Result::ok) {
                    {
                        let mut guard = stderr_text.lock().unwrap();
                        guard.push_str(&line);
                        guard.push('\n');
                        if guard.len() > 8000 {
                            let cut = guard.len() - 8000;
                            guard.drain(..cut);
                        }
                    }
                }
            }
        }));
    }

    // Poll for completion so cancellation and progress parsing stay responsive.
    // (whisper-cli prints segment lines; parsing them for progress requires the
    // lines, so we parse the accumulated stderr after exit as a fallback and
    // rely on the percent scan during the run through the shared buffer.)
    let progress_buffer = Arc::clone(&stderr_text);
    let mut last_percent = -1i32;
    let status = loop {
        if cancelled.load(Ordering::SeqCst) {
            let _ = child.kill();
        }
        // Scan newly accumulated output for progress markers.
        let snapshot = progress_buffer.lock().unwrap().clone();
        parse_progress_chunk(&snapshot, duration, &mut last_percent, on_progress);
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(120)),
            Err(e) => return Err(e.to_string()),
        }
    };
    *child_pid.lock().unwrap() = None;
    for handle in handles {
        let _ = handle.join();
    }

    if cancelled.load(Ordering::SeqCst) {
        return Err("cancelled".to_string());
    }
    let json_path = output_prefix.with_extension("json");
    if !status.success() {
        let detail = stderr_text
            .lock()
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" ");
        let _ = std::fs::remove_file(&json_path);
        return Err(if detail.is_empty() {
            format!("El motor terminó con código {:?}.", status.code())
        } else {
            detail
        });
    }
    if !json_path.exists() {
        return Err("El motor no generó resultados.".to_string());
    }
    let raw = std::fs::read_to_string(&json_path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&json_path);
    let result: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    on_progress(100);

    let utterances = extract_utterances(&result);
    let duration_out = utterances.iter().map(|u| u.end).fold(0.0, f64::max);
    let detected = result
        .get("language")
        .or_else(|| result.pointer("/result/language"))
        .or_else(|| result.pointer("/params/language"))
        .and_then(|v| v.as_str())
        .unwrap_or(language)
        .to_string();

    Ok(TranscribeResult {
        words: extract_words(&result),
        utterances,
        duration: duration_out,
        detected_language: detected,
    })
}

pub fn kill_child(child_pid: &Arc<Mutex<Option<u32>>>) {
    if let Some(pid) = *child_pid.lock().unwrap() {
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F", "/T"])
                .status();
        }
        #[cfg(not(windows))]
        {
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_words_subword_tokens() {
        let result = json!({
            "transcription": [{
                "text": " Hola bienvenidos al tutorial de hoy.",
                "timestamps": { "from": "00:00:00,000", "to": "00:00:05,120" },
                "tokens": [
                    { "text": "[_BEG_]", "timestamps": { "from": "00:00:00,000", "to": "00:00:00,000" } },
                    { "text": " Hola", "timestamps": { "from": "00:00:00,010", "to": "00:00:00,260" } },
                    { "text": " bien", "timestamps": { "from": "00:00:00,260", "to": "00:00:00,500" } },
                    { "text": "venidos", "timestamps": { "from": "00:00:00,540", "to": "00:00:00,980" } },
                    { "text": " al", "timestamps": { "from": "00:00:00,980", "to": "00:00:01,110" } },
                    { "text": " tutorial", "timestamps": { "from": "00:00:01,110", "to": "00:00:01,640" } },
                    { "text": " hoy.", "timestamps": { "from": "00:00:04,900", "to": "00:00:05,090" } },
                    { "text": "[_TT_256]", "timestamps": { "from": "00:00:05,120", "to": "00:00:05,120" } }
                ]
            }]
        });
        let words = extract_words(&result);
        let got: Vec<(&str, f64, f64)> = words
            .iter()
            .map(|w| (w.text.as_str(), w.start, w.end))
            .collect();
        assert_eq!(
            got,
            vec![
                ("Hola", 0.01, 0.26),
                ("bienvenidos", 0.26, 0.98),
                ("al", 0.98, 1.11),
                ("tutorial", 1.11, 1.64),
                ("hoy.", 4.9, 5.09)
            ]
        );
        assert_eq!(
            extract_utterances(&result)[0].text,
            "Hola bienvenidos al tutorial de hoy."
        );
    }

    #[test]
    #[ignore = "requires network"]
    fn download_url_reports_progress() {
        let destination = std::env::temp_dir().join("panto-srt-dl-test.json");
        let progress = std::sync::Mutex::new(Vec::new());
        download_url(
            "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3/resolve/main/tokenizer.json?download=true",
            &destination,
            Some(1_171_000),
            &|percent| progress.lock().unwrap().push(percent),
        )
        .expect("download should succeed");
        let values = progress.lock().unwrap().clone();
        let _ = std::fs::remove_file(&destination);
        assert!(
            values.len() > 1,
            "expected several progress updates, got {values:?}"
        );
        assert!(
            values.windows(2).all(|pair| pair[1] >= pair[0]),
            "progress must be monotonic: {values:?}"
        );
        assert_eq!(*values.last().unwrap(), 100);
    }

    #[test]
    fn sha256_file_matches_known_digest() {
        let path = std::env::temp_dir().join(format!("panto-sha-{}", std::process::id()));
        std::fs::write(&path, b"panto").unwrap();
        let digest = sha256_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            digest,
            "5b16cb2218be5516b8a0c73c8be27da10bf4058564c959501b454bf0726e4def"
        );
    }

    #[test]
    fn download_rejects_sha256_mismatch() {
        let source = std::env::temp_dir().join(format!("panto-dl-src-{}", std::process::id()));
        let destination = std::env::temp_dir().join(format!("panto-dl-dst-{}", std::process::id()));
        std::fs::write(&source, b"panto").unwrap();
        let result = download_url_cancellable(
            &format!("file://{}", source.to_string_lossy()),
            &destination,
            Some(5),
            Some("0000000000000000000000000000000000000000000000000000000000000000"),
            &|_| {},
            None,
            None,
        );
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&destination);
        assert!(result.is_err());
        assert!(!destination.exists());
    }

    #[test]
    fn extract_words_segment_fallback() {
        let result = json!({ "transcription": [
            { "text": " Solo texto.", "timestamps": { "from": "00:00:01,000", "to": "00:00:02,500" } }
        ]});
        let words = extract_words(&result);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "Solo texto.");
        assert_eq!(words[0].start, 1.0);
        assert_eq!(words[0].end, 2.5);
    }
}
