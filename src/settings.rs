//! Persistent user settings and OS auto-launch management.

use auto_launch::AutoLaunchBuilder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::platform::app_config_dir;

fn default_shortcut() -> String {
    "Alt+Q".to_string()
}

fn default_ocr_shortcut() -> String {
    "Alt+S".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub show_floating_badge: bool,
    pub enable_double_copy: bool,
    pub clean_pdf_line_breaks: bool,
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    #[serde(default = "default_ocr_shortcut")]
    pub ocr_shortcut: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            show_floating_badge: true,
            enable_double_copy: true,
            clean_pdf_line_breaks: true,
            shortcut: default_shortcut(),
            ocr_shortcut: default_ocr_shortcut(),
        }
    }
}

impl AppSettings {
    fn config_path() -> Option<PathBuf> {
        app_config_dir().map(|p| p.join("settings.json"))
    }

    pub fn load() -> Self {
        Self::config_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|raw| serde_json::from_str::<Self>(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(path, json);
            }
        }
    }

    fn build_auto_launch() -> Option<auto_launch::AutoLaunch> {
        let exe = std::env::current_exe().ok()?;
        let exe_str = exe.to_str()?;
        let mut builder = AutoLaunchBuilder::new();
        builder.set_app_name("QuickTranslateVN");
        builder.set_app_path(exe_str);
        #[cfg(target_os = "macos")]
        {
            builder.set_use_launch_agent(true);
        }
        builder.build().ok()
    }

    pub fn is_startup_enabled() -> bool {
        Self::build_auto_launch()
            .and_then(|al| al.is_enabled().ok())
            .unwrap_or(false)
    }

    pub fn set_startup_enabled(enable: bool) {
        if let Some(al) = Self::build_auto_launch() {
            if enable {
                let _ = al.enable();
            } else {
                let _ = al.disable();
            }
        }
    }
}
