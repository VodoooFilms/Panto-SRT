//! AI video super-resolution for the Upscale tab: engine catalog, engine
//! selection and `realesrgan-ncnn-vulkan` discovery.
//!
//! Real-ESRGAN `realesr-animevideov3` (BSD-3-Clause, SRVGGNetCompact) is a
//! compact ~1.2 MB-per-scale model for animated/video content; the weights and
//! the MIT ncnn-vulkan runner ship inside the app bundle, so nothing is ever
//! downloaded.

use crate::whisper::EnginePaths;
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const LANCZOS_ID: &str = "lanczos";
pub const ESR_X2_ID: &str = "esr-x2";
pub const ESR_X4_ID: &str = "esr-x4";

pub struct VsrModelDef {
    pub id: &'static str,
    pub name: &'static str,
    pub engine_label: &'static str,
    pub factor: u32,
}

pub const CATALOG: &[VsrModelDef] = &[
    VsrModelDef {
        id: ESR_X2_ID,
        name: "Real-ESRGAN 2×",
        engine_label: "IA · ncnn (GPU)",
        factor: 2,
    },
    VsrModelDef {
        id: ESR_X4_ID,
        name: "Real-ESRGAN 4×",
        engine_label: "IA · ncnn (GPU)",
        factor: 4,
    },
];

/// Weight files per scale (shipped in `Resources/esrgan/models/`).
fn model_files(factor: u32) -> [String; 2] {
    [
        format!("realesr-animevideov3-x{factor}.param"),
        format!("realesr-animevideov3-x{factor}.bin"),
    ]
}

/// Locate the bundled `realesrgan-ncnn-vulkan` runner.
pub fn find_realesrgan(paths: &EnginePaths) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        "realesrgan-ncnn-vulkan.exe"
    } else {
        "realesrgan-ncnn-vulkan"
    };
    let mut candidates = Vec::new();
    if let Some(res) = &paths.resource_dir {
        candidates.push(res.join("esrgan").join(exe));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = manifest.parent() {
        candidates.push(root.join("build").join("esrgan").join(exe));
    }
    candidates.into_iter().find(|c| c.exists())
}

pub fn models_dir(paths: &EnginePaths) -> Option<PathBuf> {
    let relative = Path::new("esrgan").join("models");
    if let Some(res) = &paths.resource_dir {
        let dir = res.join(&relative);
        if dir.is_dir() {
            return Some(dir);
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = manifest.parent() {
        let dir = root.join("build").join("esrgan").join("models");
        if dir.is_dir() {
            return Some(dir);
        }
    }
    None
}

/// An engine is usable only when both the runner and its weights are present.
pub fn available(model: &VsrModelDef, paths: &EnginePaths) -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    find_realesrgan(paths).is_some()
        && models_dir(paths).is_some_and(|dir| {
            model_files(model.factor).iter().all(|f| dir.join(f).exists())
        })
}

pub fn def(paths: &EnginePaths, id: &str) -> Option<&'static VsrModelDef> {
    CATALOG
        .iter()
        .find(|m| m.id == id && available(m, paths))
}

fn settings_path(paths: &EnginePaths) -> PathBuf {
    paths.app_data_dir.join("upscale-settings.json")
}

pub fn active_engine(paths: &EnginePaths) -> String {
    if let Ok(raw) = std::fs::read_to_string(settings_path(paths)) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(id) = value.get("upscaleEngine").and_then(|v| v.as_str()) {
                // "relay-x2" from the previous RelayVSR build maps onto the
                // same 2× slot so the AI choice survives upgrades.
                let id = match id {
                    "relay-x2" => ESR_X2_ID,
                    "relay-x4" => ESR_X4_ID,
                    other => other,
                };
                if id == LANCZOS_ID {
                    return id.to_string();
                }
                if def(paths, id).is_some() {
                    return id.to_string();
                }
            }
        }
    }
    LANCZOS_ID.to_string()
}

pub fn set_active_engine(paths: &EnginePaths, id: &str) -> Result<(), String> {
    if id != LANCZOS_ID && def(paths, id).is_none() {
        return Err(format!("Motor desconocido: {id}"));
    }
    let _ = std::fs::create_dir_all(&paths.app_data_dir);
    std::fs::write(
        settings_path(paths),
        serde_json::json!({ "upscaleEngine": id }).to_string(),
    )
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleModelInfo {
    pub id: String,
    pub name: String,
    pub engine_label: String,
    pub size: u64,
    pub downloaded: bool,
    pub active: bool,
    pub builtin: bool,
}

pub fn list_models(paths: &EnginePaths) -> Vec<UpscaleModelInfo> {
    let active = active_engine(paths);
    let mut list = vec![UpscaleModelInfo {
        id: LANCZOS_ID.to_string(),
        name: "Lanczos".to_string(),
        engine_label: "FFmpeg · integrado".to_string(),
        size: 0,
        downloaded: true,
        active: active == LANCZOS_ID,
        builtin: true,
    }];
    for model in CATALOG.iter().filter(|m| available(m, paths)) {
        let size = files(paths, model)
            .iter()
            .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
            .sum();
        list.push(UpscaleModelInfo {
            id: model.id.to_string(),
            name: model.name.to_string(),
            engine_label: model.engine_label.to_string(),
            size,
            downloaded: true,
            active: active == model.id,
            builtin: true,
        });
    }
    list
}

/// Weight file paths for a model (param + bin, in the shipped models dir).
pub fn files(paths: &EnginePaths, model: &VsrModelDef) -> Vec<PathBuf> {
    let dir = models_dir(paths).unwrap_or_default();
    model_files(model.factor)
        .iter()
        .map(|f| dir.join(f))
        .collect()
}
