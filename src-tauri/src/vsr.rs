//! RelayVSR (AI video super-resolution) for the Upscale tab: model catalog,
//! engine selection and `sd-cli` (stable-diffusion.cpp) discovery.
//!
//! Weights are the Comfy-Org repackaged checkpoints (Apache-2.0), downloaded
//! on demand into the same `models` folder the Subtitles engines use.

use crate::whisper::EnginePaths;
use serde::Serialize;
use std::path::PathBuf;

pub const LANCZOS_ID: &str = "lanczos";

pub struct VsrFile {
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

pub struct VsrModelDef {
    pub id: &'static str,
    pub name: &'static str,
    pub engine_label: &'static str,
    pub factor: u32,
    pub files: &'static [VsrFile],
}

const X2_FILES: &[VsrFile] = &[
    VsrFile {
        file: "relayvsr_x2_diffusion.safetensors",
        url: "https://huggingface.co/Comfy-Org/RelayVSR/resolve/main/diffusion_models/relayvsr_x2_lightx2v_int8_convrot.safetensors?download=true",
        size: 5_163_295_248,
        sha256: "240e75e0c17cf2845d559b25254a230ff7458f81449d7a8c5e7e3fb8275c7b3c",
    },
    VsrFile {
        file: "relayvsr_x2_flash.safetensors",
        url: "https://huggingface.co/Comfy-Org/RelayVSR/resolve/main/vae/relayvsr_flash_vae_x2_fp32.safetensors?download=true",
        size: 127_759_464,
        sha256: "b5bb0339acb5b52545911ea29a51b4c5b805776a779222b5a14591571f42e21c",
    },
];

const X4_FILES: &[VsrFile] = &[
    VsrFile {
        file: "relayvsr_x4_diffusion.safetensors",
        url: "https://huggingface.co/Comfy-Org/RelayVSR/resolve/main/diffusion_models/relayvsr_x4_int8_convrot.safetensors?download=true",
        size: 5_164_179_952,
        sha256: "21303b3c5f9ab4e1fd3fe00df5dd2e0b343f0cf2e3d269e03a9872000a313a2e",
    },
    VsrFile {
        file: "relayvsr_x4_flash.safetensors",
        url: "https://huggingface.co/Comfy-Org/RelayVSR/resolve/main/vae/relayvsr_flash_vae_x4_fp32.safetensors?download=true",
        size: 127_759_784,
        sha256: "7b8b8c99275fd5c4501b0589cc7b4c4506456a5df83160ef4e207e147b30f714",
    },
];

pub const CATALOG: &[VsrModelDef] = &[
    VsrModelDef {
        id: "relay-x2",
        name: "RelayVSR 2×",
        engine_label: "IA · sd.cpp (Metal)",
        factor: 2,
        files: X2_FILES,
    },
    VsrModelDef {
        id: "relay-x4",
        name: "RelayVSR 4×",
        engine_label: "IA · sd.cpp (Metal)",
        factor: 4,
        files: X4_FILES,
    },
];

/// RelayVSR via stable-diffusion.cpp is currently shipped for macOS only.
pub fn available(model: &VsrModelDef) -> bool {
    let _ = model;
    cfg!(target_os = "macos")
}

pub fn def(id: &str) -> Option<&'static VsrModelDef> {
    CATALOG.iter().find(|m| m.id == id && available(m))
}

pub fn total_size(model: &VsrModelDef) -> u64 {
    model.files.iter().map(|f| f.size).sum()
}

pub fn is_downloaded(paths: &EnginePaths, model: &VsrModelDef) -> bool {
    let dir = crate::whisper::models_dir(paths);
    model.files.iter().all(|f| dir.join(f.file).exists())
}

pub fn file_paths(paths: &EnginePaths, model: &VsrModelDef) -> Vec<PathBuf> {
    let dir = crate::whisper::models_dir(paths);
    model.files.iter().map(|f| dir.join(f.file)).collect()
}

fn settings_path(paths: &EnginePaths) -> PathBuf {
    paths.app_data_dir.join("upscale-settings.json")
}

pub fn active_engine(paths: &EnginePaths) -> String {
    if let Ok(raw) = std::fs::read_to_string(settings_path(paths)) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(id) = value.get("upscaleEngine").and_then(|v| v.as_str()) {
                if id == LANCZOS_ID || def(id).is_some() {
                    return id.to_string();
                }
            }
        }
    }
    LANCZOS_ID.to_string()
}

pub fn set_active_engine(paths: &EnginePaths, id: &str) -> Result<(), String> {
    if id != LANCZOS_ID && def(id).is_none() {
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
    for model in CATALOG.iter().filter(|m| available(m)) {
        let downloaded = is_downloaded(paths, model);
        let size = if downloaded {
            file_paths(paths, model)
                .iter()
                .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
                .sum()
        } else {
            total_size(model)
        };
        list.push(UpscaleModelInfo {
            id: model.id.to_string(),
            name: model.name.to_string(),
            engine_label: model.engine_label.to_string(),
            size,
            downloaded,
            active: active == model.id,
            builtin: false,
        });
    }
    list
}

pub fn delete_model(paths: &EnginePaths, id: &str) -> Result<(), String> {
    let model = def(id).ok_or_else(|| format!("Modelo desconocido: {id}"))?;
    for path in file_paths(paths, model) {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        let _ = std::fs::remove_file(path.with_extension("bin.part"));
    }
    if active_engine(paths) == id {
        set_active_engine(paths, LANCZOS_ID)?;
    }
    Ok(())
}

/// Locate the bundled `sd-cli` (stable-diffusion.cpp) binary.
pub fn find_sd_cli(paths: &EnginePaths) -> Option<PathBuf> {
    let exe = if cfg!(windows) { "sd-cli.exe" } else { "sd-cli" };
    let mut candidates = Vec::new();
    if let Some(res) = &paths.resource_dir {
        candidates.push(res.join("sd").join(exe));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = manifest.parent() {
        candidates.push(root.join("build").join("sd").join(exe));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin/sd-cli"));
    candidates.push(PathBuf::from("/usr/local/bin/sd-cli"));
    candidates.into_iter().find(|c| c.exists())
}
