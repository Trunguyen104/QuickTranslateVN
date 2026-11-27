use auto_launch::AutoLaunchBuilder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub show_floating_badge: bool,
    pub enable_double_copy: bool,
    pub clean_pdf_line_breaks: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            show_floating_badge: true,
            enable_double_copy: true,
            clean_pdf_line_breaks: true,
        }
    }
}

impl AppSettings {
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|mut p| {
            p.push("QuickTranslateVN");
            p.push("settings.json");
            p
        })
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(parsed) = serde_json::from_str::<AppSettings>(&content) {
                    return parsed;
                }
            }
        }
        Self::default()
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
        AutoLaunchBuilder::new()
            .set_app_name("QuickTranslateVN")
            .set_app_path(exe_str)
            .set_use_launch_agent(true)
            .build()
            .ok()
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
