use arboard::Clipboard;
use std::thread;
use std::time::Duration;

/// Reads text already placed on the clipboard (e.g. after user's own Double Ctrl+C / Cmd+C).
pub fn read_current_clipboard() -> Option<String> {
    thread::sleep(Duration::from_millis(45));
    let mut cb = Clipboard::new().ok()?;
    let text = cb.get_text().ok()?;
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Captures currently highlighted text across any application on Windows, macOS, or Linux,
/// while preserving the user's previous clipboard contents.
pub fn capture_selected_text() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::capture_selected_text_win()
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::capture_selected_text_mac()
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::capture_selected_text_linux()
    }
}

/// Replaces currently highlighted text inside the active editor with `replacement`,
/// restoring the user's original clipboard afterwards.
pub fn replace_selected_text(replacement: &str) -> bool {
    if replacement.is_empty() {
        return false;
    }
    #[cfg(target_os = "windows")]
    {
        windows_impl::replace_selected_text_win(replacement)
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::replace_selected_text_mac(replacement)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::replace_selected_text_linux(replacement)
    }
}

// ============================================================================
// WINDOWS IMPLEMENTATION
// ============================================================================
#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use std::ffi::OsString;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStringExt;
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardSequenceNumber, OpenClipboard,
        RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        VK_C, VK_CONTROL, VK_MENU, VK_SHIFT, VK_V,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    const CF_UNICODETEXT: u32 = 13;

    const TERMINALS: &[&str] = &[
        "cmd.exe",
        "powershell.exe",
        "pwsh.exe",
        "windowsterminal.exe",
        "mintty.exe",
        "wsl.exe",
        "alacritty.exe",
        "wezterm-gui.exe",
        "conhost.exe",
    ];

    fn is_foreground_terminal() -> bool {
        unsafe {
            let hwnd: HWND = GetForegroundWindow();
            if hwnd.is_null() {
                return false;
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 {
                return false;
            }
            let hproc: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if hproc.is_null() {
                return false;
            }
            let mut buf = [0u16; 512];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(hproc, 0, buf.as_mut_ptr(), &mut size);
            CloseHandle(hproc);
            if ok == 0 || size == 0 {
                return false;
            }
            let path = OsString::from_wide(&buf[..size as usize])
                .to_string_lossy()
                .to_lowercase();
            TERMINALS.iter().any(|t| path.ends_with(t))
        }
    }

    fn make_key_input(vk: u16, key_up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if key_up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send_clean_ctrl_combo(vk_letter: u16) {
        unsafe {
            // Wait briefly if user is still releasing Alt/Shift from Alt+Q
            for _ in 0..10 {
                let alt_down = (GetAsyncKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
                let shift_down = (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
                if !alt_down && !shift_down {
                    break;
                }
                thread::sleep(Duration::from_millis(14));
            }

            let mut inputs: Vec<INPUT> = Vec::with_capacity(6);
            if (GetAsyncKeyState(VK_MENU as i32) as u16 & 0x8000) != 0 {
                inputs.push(make_key_input(VK_MENU, true));
            }
            if (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0 {
                inputs.push(make_key_input(VK_SHIFT, true));
            }

            inputs.push(make_key_input(VK_CONTROL, false));
            inputs.push(make_key_input(vk_letter, false));
            inputs.push(make_key_input(vk_letter, true));
            inputs.push(make_key_input(VK_CONTROL, true));

            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                size_of::<INPUT>() as i32,
            );
        }
    }

    fn set_clipboard_silent_win(text: Option<&str>) {
        unsafe {
            for _ in 0..4 {
                if OpenClipboard(null_mut()) != 0 {
                    EmptyClipboard();

                    if let Some(s) = text {
                        if !s.is_empty() {
                            let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
                            let bytes = wide.len() * 2;
                            let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes);
                            if !hmem.is_null() {
                                let dst = GlobalLock(hmem) as *mut u16;
                                if !dst.is_null() {
                                    std::ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
                                    GlobalUnlock(hmem);
                                    SetClipboardData(CF_UNICODETEXT as u32, hmem);
                                }
                            }
                        }
                    }

                    // Exclude from Windows Win+V Clipboard History and Cloud Clipboard
                    set_dword_format("ExcludeClipboardContentFromMonitorProcessing\0", 1);
                    set_dword_format("CanIncludeInClipboardHistory\0", 0);
                    set_dword_format("CanUploadToCloudClipboard\0", 0);

                    CloseClipboard();
                    break;
                }
                thread::sleep(Duration::from_millis(12));
            }
        }
    }

    unsafe fn set_dword_format(name_nul: &str, val: u32) {
        let wide_name: Vec<u16> = name_nul.encode_utf16().collect();
        let fmt = RegisterClipboardFormatW(wide_name.as_ptr());
        if fmt != 0 {
            let hmem = GlobalAlloc(GMEM_MOVEABLE, size_of::<u32>());
            if !hmem.is_null() {
                let ptr = GlobalLock(hmem) as *mut u32;
                if !ptr.is_null() {
                    *ptr = val;
                    GlobalUnlock(hmem);
                    SetClipboardData(fmt, hmem);
                }
            }
        }
    }

    pub fn capture_selected_text_win() -> Option<String> {
        if is_foreground_terminal() {
            return None;
        }

        let backup = Clipboard::new().ok().and_then(|mut c| c.get_text().ok());
        let seq_before = unsafe { GetClipboardSequenceNumber() };

        send_clean_ctrl_combo(VK_C);

        let mut changed = false;
        for _ in 0..22 {
            thread::sleep(Duration::from_millis(12));
            if unsafe { GetClipboardSequenceNumber() } != seq_before {
                changed = true;
                break;
            }
        }

        if !changed {
            return None;
        }

        thread::sleep(Duration::from_millis(14));

        let captured = Clipboard::new().ok().and_then(|mut c| c.get_text().ok());
        set_clipboard_silent_win(backup.as_deref());

        let cleaned = captured?.trim_end_matches(&['\r', '\n', '\t'][..]).trim().to_string();
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    }

    pub fn replace_selected_text_win(replacement: &str) -> bool {
        if is_foreground_terminal() {
            return false;
        }
        let backup = Clipboard::new().ok().and_then(|mut c| c.get_text().ok());
        set_clipboard_silent_win(Some(replacement));
        thread::sleep(Duration::from_millis(25));
        send_clean_ctrl_combo(VK_V);
        thread::sleep(Duration::from_millis(75));
        set_clipboard_silent_win(backup.as_deref());
        let _ = unsafe { zeroed::<usize>() };
        true
    }
}

// ============================================================================
// MACOS IMPLEMENTATION
// ============================================================================
#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use std::process::Command;

    const TERMINALS: &[&str] = &["Terminal", "iTerm2", "Alacritty", "WezTerm", "kitty"];

    fn is_frontmost_terminal() -> bool {
        let out = Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get name of first application process whose frontmost is true",
            ])
            .output();
        if let Ok(o) = out {
            let name = String::from_utf8_lossy(&o.stdout);
            let trimmed = name.trim();
            return TERMINALS.iter().any(|t| trimmed.eq_ignore_ascii_case(t));
        }
        false
    }

    pub fn capture_selected_text_mac() -> Option<String> {
        if is_frontmost_terminal() {
            return None;
        }
        let mut cb = Clipboard::new().ok()?;
        let backup = cb.get_text().ok();
        let _ = cb.set_text(String::new());

        let _ = Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to keystroke \"c\" using command down",
            ])
            .output();

        thread::sleep(Duration::from_millis(90));
        let captured = cb.get_text().ok();

        if let Some(old) = backup {
            let _ = cb.set_text(old);
        }

        let trimmed = captured?.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    pub fn replace_selected_text_mac(replacement: &str) -> bool {
        if is_frontmost_terminal() {
            return false;
        }
        let Ok(mut cb) = Clipboard::new() else {
            return false;
        };
        let backup = cb.get_text().ok();
        if cb.set_text(replacement.to_string()).is_err() {
            return false;
        }
        thread::sleep(Duration::from_millis(30));
        let _ = Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to keystroke \"v\" using command down",
            ])
            .output();
        thread::sleep(Duration::from_millis(80));
        if let Some(old) = backup {
            let _ = cb.set_text(old);
        }
        true
    }
}

// ============================================================================
// LINUX IMPLEMENTATION (Primary Selection + X11/Wayland Fallback)
// ============================================================================
#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use arboard::{GetExtLinux, LinuxClipboardKind};
    use std::process::Command;

    pub fn capture_selected_text_linux() -> Option<String> {
        let mut cb = Clipboard::new().ok()?;

        // 1. Native X11/Wayland Primary Selection (zero keystrokes, zero clipboard pollution!)
        if let Ok(primary) = cb.get().clipboard(LinuxClipboardKind::Primary).text() {
            let t = primary.trim().to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }

        // 2. Fallback to simulated Ctrl+C via xdotool (X11) or wtype (Wayland)
        let backup = cb.get_text().ok();
        let _ = cb.set_text(String::new());

        let copied = Command::new("xdotool")
            .args(["key", "--clearmodifiers", "ctrl+c"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
            || Command::new("wtype")
                .args(["-M", "ctrl", "c", "-m", "ctrl"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

        if !copied {
            if let Some(old) = backup {
                let _ = cb.set_text(old);
            }
            return None;
        }

        thread::sleep(Duration::from_millis(80));
        let captured = cb.get_text().ok();
        if let Some(old) = backup {
            let _ = cb.set_text(old);
        }

        let trimmed = captured?.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    pub fn replace_selected_text_linux(replacement: &str) -> bool {
        let Ok(mut cb) = Clipboard::new() else {
            return false;
        };
        let backup = cb.get_text().ok();
        if cb.set_text(replacement.to_string()).is_err() {
            return false;
        }
        thread::sleep(Duration::from_millis(30));

        let pasted = Command::new("xdotool")
            .args(["key", "--clearmodifiers", "ctrl+v"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
            || Command::new("wtype")
                .args(["-M", "ctrl", "v", "-m", "ctrl"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

        thread::sleep(Duration::from_millis(80));
        if let Some(old) = backup {
            let _ = cb.set_text(old);
        }
        pasted
    }
}
