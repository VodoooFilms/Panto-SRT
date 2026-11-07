//! Small helpers for the ffmpeg-backed Convertir and Upscale tabs: track the
//! running child process and stop it from another command.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct ActiveProcess {
    pub pid: u32,
    pub cancel: Arc<AtomicBool>,
}

pub type ActiveSlot = Arc<Mutex<Option<ActiveProcess>>>;

/// Terminate a child process (and its children) on every supported platform.
pub fn terminate(pid: u32) {
    if pid == 0 {
        return;
    }

    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    }

    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
    }
}

/// Mark the active process as cancelled and kill it.
pub fn cancel(slot: &ActiveSlot) -> bool {
    let Ok(guard) = slot.lock() else {
        return false;
    };
    let Some(active) = guard.as_ref() else {
        return false;
    };
    active.cancel.store(true, Ordering::SeqCst);
    terminate(active.pid);
    true
}

/// A short, quiet chime when a conversion finishes (macOS / Linux).
pub fn play_completion_sound() {
    #[cfg(target_os = "macos")]
    {
        let path = "/System/Library/Sounds/Tink.aiff";
        if std::path::Path::new(path).exists() {
            let _ = std::process::Command::new("/usr/bin/afplay")
                .args(["-v", "0.35", path])
                .spawn();
        }
    }

    #[cfg(target_os = "linux")]
    {
        let player = "/usr/bin/canberra-gtk-play";
        if std::path::Path::new(player).exists() {
            let _ = std::process::Command::new(player)
                .args(["-i", "complete", "-V", "-9.0"])
                .spawn();
        }
    }
}
