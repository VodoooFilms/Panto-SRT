//! Upscale targets, fit modes and size estimates for the Upscale tab.

use serde::Serialize;

#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub id: &'static str,
    pub label: &'static str,
    pub width: u32,
    pub height: u32,
}

pub const TARGETS: [Target; 4] = [
    Target {
        id: "720",
        label: "720p",
        width: 1280,
        height: 720,
    },
    Target {
        id: "1080",
        label: "1080p",
        width: 1920,
        height: 1080,
    },
    Target {
        id: "1440",
        label: "1440p",
        width: 2560,
        height: 1440,
    },
    Target {
        id: "2160",
        label: "4K",
        width: 3840,
        height: 2160,
    },
];

pub fn target_by_id(id: &str) -> Target {
    TARGETS
        .iter()
        .copied()
        .find(|target| target.id == id)
        .unwrap_or(TARGETS[1])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Pad,
    Crop,
    Stretch,
}

impl Mode {
    pub fn parse(value: &str) -> Self {
        match value {
            "pad" => Mode::Pad,
            "stretch" => Mode::Stretch,
            _ => Mode::Crop,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Pad => "pad",
            Mode::Crop => "crop",
            Mode::Stretch => "stretch",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub target_id: String,
    pub label: String,
    pub mode: String,
    pub target_width: u32,
    pub target_height: u32,
    pub scaled_width: u32,
    pub scaled_height: u32,
    pub bar_left: u32,
    pub bar_right: u32,
    pub bar_top: u32,
    pub bar_bottom: u32,
    pub crop_left: u32,
    pub crop_top: u32,
    pub estimated_bytes: u64,
    pub valid: bool,
}

pub fn compute_plan(
    width: u32,
    height: u32,
    duration: f64,
    audio_kbps: u32,
    target_id: &str,
    mode: &str,
) -> Plan {
    let target = target_by_id(target_id);
    let mode = Mode::parse(mode);

    let mut plan = Plan {
        target_id: target.id.to_string(),
        label: target.label.to_string(),
        mode: mode.as_str().to_string(),
        target_width: target.width,
        target_height: target.height,
        scaled_width: target.width,
        scaled_height: target.height,
        bar_left: 0,
        bar_right: 0,
        bar_top: 0,
        bar_bottom: 0,
        crop_left: 0,
        crop_top: 0,
        estimated_bytes: estimate_bytes(target.height, duration, audio_kbps),
        valid: width > 0 && height > 0,
    };

    if !plan.valid {
        return plan;
    }

    match mode {
        Mode::Pad => {
            let scale = f64::min(
                target.width as f64 / width as f64,
                target.height as f64 / height as f64,
            );
            let scaled_width = floor_even(width as f64 * scale).min(target.width);
            let scaled_height = floor_even(height as f64 * scale).min(target.height);

            let horizontal = target.width.saturating_sub(scaled_width);
            let vertical = target.height.saturating_sub(scaled_height);

            plan.scaled_width = scaled_width;
            plan.scaled_height = scaled_height;
            plan.bar_left = horizontal / 2;
            plan.bar_right = horizontal - horizontal / 2;
            plan.bar_top = vertical / 2;
            plan.bar_bottom = vertical - vertical / 2;
        }
        Mode::Crop => {
            let scale = f64::max(
                target.width as f64 / width as f64,
                target.height as f64 / height as f64,
            );
            let scaled_width = ceil_even(width as f64 * scale).max(target.width);
            let scaled_height = ceil_even(height as f64 * scale).max(target.height);

            plan.scaled_width = scaled_width;
            plan.scaled_height = scaled_height;
            plan.crop_left = (scaled_width - target.width) / 2;
            plan.crop_top = (scaled_height - target.height) / 2;
        }
        Mode::Stretch => {
            plan.scaled_width = target.width;
            plan.scaled_height = target.height;
        }
    }

    plan
}

pub fn build_filter(plan: &Plan, mode: Mode) -> String {
    let width = plan.target_width;
    let height = plan.target_height;
    let flags = "lanczos+accurate_rnd+full_chroma_int";

    match mode {
        Mode::Pad => format!(
            "scale={}:{}:flags={flags},pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:black,setsar=1",
            plan.scaled_width, plan.scaled_height
        ),
        Mode::Crop => format!(
            "scale={}:{}:flags={flags},crop={width}:{height}:{}:{},setsar=1",
            plan.scaled_width, plan.scaled_height, plan.crop_left, plan.crop_top
        ),
        Mode::Stretch => format!("scale={width}:{height}:flags={flags},setsar=1"),
    }
}

pub fn estimate_bytes(target_height: u32, duration: f64, audio_kbps: u32) -> u64 {
    if duration <= 0.0 {
        return 0;
    }

    let video_kbps = match target_height {
        720 => 5_500.0,
        1080 => 10_000.0,
        1440 => 20_000.0,
        _ => 40_000.0,
    };
    let audio_kbps = if audio_kbps == 0 {
        0.0
    } else {
        audio_kbps as f64
    };

    ((duration * (video_kbps + audio_kbps) * 1_000.0) / 8.0 * 1.02) as u64
}

fn floor_even(value: f64) -> u32 {
    let rounded = value.floor() as i64;
    let even = rounded - rounded.rem_euclid(2);
    even.max(2) as u32
}

fn ceil_even(value: f64) -> u32 {
    let rounded = value.ceil() as i64;
    let even = rounded + rounded.rem_euclid(2);
    even.max(2) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letterboxes_832x480_into_1080p_with_24px_bars() {
        let plan = compute_plan(832, 480, 60.0, 192, "1080", "pad");

        assert_eq!(plan.target_width, 1920);
        assert_eq!(plan.target_height, 1080);
        assert_eq!(plan.scaled_width, 1872);
        assert_eq!(plan.scaled_height, 1080);
        assert_eq!(plan.bar_left, 24);
        assert_eq!(plan.bar_right, 24);
        assert_eq!(plan.bar_top, 0);
        assert_eq!(plan.bar_bottom, 0);
    }

    #[test]
    fn crop_fills_the_target_and_centers_the_crop() {
        let plan = compute_plan(640, 480, 60.0, 192, "1080", "crop");

        assert_eq!(plan.scaled_width, 1920);
        assert_eq!(plan.scaled_height, 1440);
        assert_eq!(plan.crop_top, 180);
        assert_eq!(plan.crop_left, 0);
    }

    #[test]
    fn unknown_mode_falls_back_to_crop() {
        assert_eq!(Mode::parse("unknown"), Mode::Crop);
    }

    #[test]
    fn filter_uses_lanczos_and_black_padding() {
        let plan = compute_plan(832, 480, 60.0, 192, "1080", "pad");
        let filter = build_filter(&plan, Mode::Pad);

        assert!(filter.contains("flags=lanczos"));
        assert!(filter.contains("pad=1920:1080"));
        assert!(filter.contains("black"));
    }

    #[test]
    fn estimates_a_reasonable_size() {
        let bytes = estimate_bytes(1080, 60.0, 192);
        assert!(bytes > 70_000_000);
        assert!(bytes < 90_000_000);
    }
}
