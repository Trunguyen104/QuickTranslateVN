//! Zero-dependency OS utilities: single-instance guard, standard config/cache directories,
//! and RFC-3986 URL percent-encoding.

use std::path::PathBuf;

/// Ensures only a single instance of QuickTranslateVN runs per user session.
pub struct SingleInstanceGuard {
    #[cfg(target_os = "windows")]
    handle: windows_sys::Win32::Foundation::HANDLE,
    #[cfg(not(target_os = "windows"))]
    lock_path: PathBuf,
}

impl SingleInstanceGuard {
    pub fn try_acquire() -> Option<Self> {
        #[cfg(target_os = "windows")]
        unsafe {
            use std::ptr::null;
            use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
            use windows_sys::Win32::System::Threading::CreateMutexW;

            let name: Vec<u16> = "Local\\QuickTranslateVN_SingleInstance_Mutex\0"
                .encode_utf16()
                .collect();
            let handle = CreateMutexW(null(), 1, name.as_ptr());
            if handle.is_null() {
                return None;
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                windows_sys::Win32::Foundation::CloseHandle(handle);
                return None;
            }
            Some(Self { handle })
        }

        #[cfg(not(target_os = "windows"))]
        {
            use std::fs::OpenOptions;
            use std::io::Write;

            let lock_path = std::env::temp_dir().join("quick-translate-vn.lock");
            if let Ok(existing_pid_str) = std::fs::read_to_string(&lock_path) {
                if let Ok(pid) = existing_pid_str.trim().parse::<u32>() {
                    // Check if process is still alive via `kill -0 <pid>`
                    let alive = std::process::Command::new("kill")
                        .args(["-0", &pid.to_string()])
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                    if alive {
                        return None;
                    }
                }
            }
            let mut f = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&lock_path)
                .ok()?;
            let _ = writeln!(f, "{}", std::process::id());
            Some(Self { lock_path })
        }
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        unsafe {
            if !self.handle.is_null() {
                windows_sys::Win32::Foundation::CloseHandle(self.handle);
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = std::fs::remove_file(&self.lock_path);
        }
    }
}

/// Returns platform-standard user configuration directory for QuickTranslateVN.
pub fn app_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("LOCALAPPDATA"))
            .map(|p| PathBuf::from(p).join("QuickTranslateVN"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| {
            PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join("QuickTranslateVN")
        })
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join("QuickTranslateVN"))
    }
}

/// Returns platform-standard local cache directory (for WebView data).
pub fn app_cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(|p| {
            PathBuf::from(p)
                .join("QuickTranslateVN")
                .join("WebViewCache")
        })
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| {
            PathBuf::from(h)
                .join("Library")
                .join("Caches")
                .join("QuickTranslateVN")
        })
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .map(|p| p.join("QuickTranslateVN"))
    }
}

/// Encodes arbitrary UTF-8 string into RFC-3986 percent-encoded query value.
pub fn percent_encode(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(input.len() * 2);
    for &b in input.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0F) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_utf8_and_reserved_chars() {
        assert_eq!(percent_encode("hello world"), "hello%20world");
        assert_eq!(percent_encode("A&B=C"), "A%26B%3DC");
        assert_eq!(percent_encode("Xin chào"), "Xin%20ch%C3%A0o");
    }
}
