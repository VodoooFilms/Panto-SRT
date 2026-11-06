//! Model catalog: the two local ASR engines the user can switch between.
//! The active choice persists in `settings.json` inside the app data folder.

use crate::whisper::{self, EnginePaths};
use serde::Serialize;
use std::path::PathBuf;

pub const WHISPER_ID: &str = "whisper-turbo";
pub const PARAKEET_ID: &str = "parakeet";

pub struct ModelDef {
    pub id: &'static str,
    pub name: &'static str,
    pub engine_label: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    /// Download size in bytes (from the publisher).
    pub size: u64,
    /// whisper.cpp model id ("turbo" / "small"); empty for Parakeet.
    pub whisper_id: &'static str,
}

pub const CATALOG: &[ModelDef] = &[
    ModelDef {
        id: WHISPER_ID,
        name: "Whisper Large v3 Turbo",
        engine_label: "Whisper.cpp",
        file: "ggml-large-v3-turbo.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin?download=true",
        size: 1_624_555_275,
        whisper_id: "turbo",
    },
    ModelDef {
        id: "whisper-small",
        name: "Whisper Small",
        engine_label: "Whisper.cpp · ligero",
        file: "ggml-small.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin?download=true",
        size: 487_601_967,
        whisper_id: "small",
    },
    ModelDef {
        id: PARAKEET_ID,
        name: "Parakeet TDT 0.6B v3",
        engine_label: "NeMo · Q8",
        file: "parakeet-tdt-0.6b-v3.q8_0.gguf",
        url: "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3/resolve/main/parakeet-tdt-0.6b-v3.q8_0.gguf?download=true",
        size: 713_975_456,
        whisper_id: "",
    },
];

pub fn def(id: &str) -> Option<&'static ModelDef> {
    CATALOG.iter().find(|m| m.id == id)
}

pub fn model_path(paths: &EnginePaths, model: &ModelDef) -> PathBuf {
    if !model.whisper_id.is_empty() {
        if let Some(found) = whisper::find_model(paths, model.whisper_id) {
            return found;
        }
    }
    whisper::models_dir(paths).join(model.file)
}

pub fn is_downloaded(paths: &EnginePaths, model: &ModelDef) -> bool {
    model_path(paths, model).exists()
}

fn settings_path(paths: &EnginePaths) -> PathBuf {
    paths.app_data_dir.join("settings.json")
}

pub fn active_model(paths: &EnginePaths) -> String {
    if let Ok(raw) = std::fs::read_to_string(settings_path(paths)) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(id) = value.get("activeModel").and_then(|v| v.as_str()) {
                if def(id).is_some() {
                    return id.to_string();
                }
            }
        }
    }
    WHISPER_ID.to_string()
}

pub fn set_active_model(paths: &EnginePaths, id: &str) -> Result<(), String> {
    if def(id).is_none() {
        return Err(format!("Modelo desconocido: {id}"));
    }
    let _ = std::fs::create_dir_all(&paths.app_data_dir);
    std::fs::write(
        settings_path(paths),
        serde_json::json!({ "activeModel": id }).to_string(),
    )
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub engine_label: String,
    pub size: u64,
    pub downloaded: bool,
    pub active: bool,
}

pub fn list_models(paths: &EnginePaths) -> Vec<ModelInfo> {
    let active = active_model(paths);
    CATALOG
        .iter()
        .map(|model| {
            let path = model_path(paths, model);
            let downloaded = path.exists();
            ModelInfo {
                id: model.id.to_string(),
                name: model.name.to_string(),
                engine_label: model.engine_label.to_string(),
                size: if downloaded {
                    std::fs::metadata(&path).map(|m| m.len()).unwrap_or(model.size)
                } else {
                    model.size
                },
                downloaded,
                active: model.id == active,
            }
        })
        .collect()
}

/// Delete a downloaded model file. If it was the active one and the other
/// model is available, the other becomes active.
pub fn delete_model(paths: &EnginePaths, id: &str) -> Result<(), String> {
    let model = def(id).ok_or_else(|| format!("Modelo desconocido: {id}"))?;
    let path = model_path(paths, model);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    // Also drop any partial download.
    let _ = std::fs::remove_file(path.with_extension("bin.part"));
    if active_model(paths) == id {
        if let Some(other) = CATALOG.iter().find(|m| m.id != id) {
            if is_downloaded(paths, other) {
                set_active_model(paths, other.id)?;
            }
        }
    }
    Ok(())
}
