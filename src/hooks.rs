use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tao::event_loop::EventLoopProxy;
use tao::window::Window;

#[derive(Debug, Clone)]
pub enum UserEvent {
    ShowBadge { x: i32, y: i32 },
    HotkeyTranslate,
    HotkeyOcr,
    DoubleCopy { x: i32, y: i32 },
    BadgeClicked,
    ExternalClick,
    EscapePressed,
    TextCaptured {
        req_id: u64,
        anchor: (i32, i32),
        cleaned_text: String,
    },
    OcrFailed {
        anchor: (i32, i32),
        error_msg: String,
    },
    TranslationCompleted { req_id: u64, payload_json: String },
    TtsAudioReady { data_uri: String },
    IpcMessage(String),
}

pub struct HookSwitches {
    pub enable_badge: AtomicBool,
    pub enable_double_copy: AtomicBool,
}

/// Returns current physical cursor position `(x, y)` on screen.
pub fn get_cursor_physical_pos() -> (i32, i32) {
    #[cfg(target_os = "windows")]
    {
        win_hooks::get_cursor_pos_win()
    }
    #[cfg(not(target_os = "windows"))]
    {
        unix_hooks::get_last_cursor_pos()
    }
}

/// Applies platform-specific non-activating / tool-window styles so showing or clicking
/// the badge or popup never steals focus from the user's active document.
pub fn configure_non_activating_window(window: &Window) {
    #[cfg(target_os = "windows")]
    {
        win_hooks::make_non_activating_win(window);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
    }
}

/// Gently trims OS working set memory after hiding popup.
pub fn trim_idle_memory() {
    #[cfg(target_os = "windows")]
    {
        win_hooks::trim_working_set_win();
    }
}

/// Starts background global mouse/keyboard listener thread for `[文A]` badge, Double Copy, and ESC.
pub fn start_global_hooks(proxy: EventLoopProxy<UserEvent>, switches: Arc<HookSwitches>) {
    #[cfg(target_os = "windows")]
    {
        win_hooks::spawn_win32_hooks(proxy, switches);
    }
    #[cfg(not(target_os = "windows"))]
    {
        unix_hooks::spawn_rdev_hooks(proxy, switches);
    }
}

// ============================================================================
// WINDOWS LOW-LEVEL HOOKS & NON-ACTIVATING WINDOW SUBCLASS
// ============================================================================
#[cfg(target_os = "windows")]
mod win_hooks {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::mem::{size_of, zeroed};
    use std::ptr::null_mut;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    use tao::platform::windows::WindowExtWindows;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId, SetProcessWorkingSetSize};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_C, VK_CONTROL, VK_ESCAPE, VK_MENU, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, CallWindowProcW, DispatchMessageW, GetCursorInfo, GetCursorPos,
        GetMessageW, GetWindowLongPtrW, GetWindowThreadProcessId, LoadCursorW, SetWindowLongPtrW,
        SetWindowsHookExW, TranslateMessage, WindowFromPoint, CURSORINFO, CURSOR_SHOWING,
        GWLP_WNDPROC, GWL_EXSTYLE, IDC_IBEAM, KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLMHF_INJECTED,
        MA_NOACTIVATE, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEWHEEL, WM_RBUTTONDOWN,
        WM_SYSKEYDOWN, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    };

    struct HookState {
        proxy: EventLoopProxy<UserEvent>,
        switches: Arc<HookSwitches>,
        current_pid: u32,
        ibeam_cursor: isize,
        left_down: bool,
        down_pt: POINT,
        down_time: Instant,
        down_was_ibeam: bool,
        last_up_pt: POINT,
        last_up_time: Instant,
        last_ctrl_c_time: Option<Instant>,
    }

    thread_local! {
        static STATE: RefCell<Option<HookState>> = const { RefCell::new(None) };
    }

    pub fn get_cursor_pos_win() -> (i32, i32) {
        unsafe {
            let mut pt: POINT = zeroed();
            if GetCursorPos(&mut pt) != 0 {
                (pt.x, pt.y)
            } else {
                (200, 200)
            }
        }
    }

    pub fn trim_working_set_win() {
        unsafe {
            let hproc = GetCurrentProcess();
            let _ = SetProcessWorkingSetSize(hproc, usize::MAX, usize::MAX);
        }
    }

    fn orig_wndprocs() -> &'static Mutex<HashMap<isize, isize>> {
        static MAP: OnceLock<Mutex<HashMap<isize, isize>>> = OnceLock::new();
        MAP.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub fn make_non_activating_win(window: &Window) {
        unsafe {
            let hwnd = window.hwnd() as HWND;
            if hwnd.is_null() {
                return;
            }
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let new_style = ex_style
                | (WS_EX_NOACTIVATE as isize)
                | (WS_EX_TOOLWINDOW as isize)
                | (WS_EX_TOPMOST as isize);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);

            let orig_proc = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
            if orig_proc != 0 {
                if let Ok(mut map) = orig_wndprocs().lock() {
                    map.insert(hwnd as isize, orig_proc);
                }
                SetWindowLongPtrW(
                    hwnd,
                    GWLP_WNDPROC,
                    non_activating_wndproc as *const () as isize,
                );
            }
        }
    }

    unsafe extern "system" fn non_activating_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_MOUSEACTIVATE {
            return MA_NOACTIVATE as LRESULT;
        }
        let orig = orig_wndprocs()
            .lock()
            .ok()
            .and_then(|m| m.get(&(hwnd as isize)).copied())
            .unwrap_or(0);
        if orig != 0 {
            let proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
                std::mem::transmute(orig);
            CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
        } else {
            0
        }
    }

    fn is_cursor_ibeam(ibeam_handle: isize) -> bool {
        unsafe {
            let mut ci: CURSORINFO = zeroed();
            ci.cbSize = size_of::<CURSORINFO>() as u32;
            GetCursorInfo(&mut ci) != 0
                && (ci.flags & CURSOR_SHOWING) != 0
                && (ci.hCursor as isize) == ibeam_handle
        }
    }

    fn is_own_window(pt: POINT, my_pid: u32) -> bool {
        unsafe {
            let hwnd = WindowFromPoint(pt);
            if hwnd.is_null() {
                return false;
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            pid == my_pid
        }
    }

    unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let msg = wparam as u32;
            if matches!(
                msg,
                WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_MOUSEWHEEL
            ) {
                let hs = &*(lparam as *const MSLLHOOKSTRUCT);
                if (hs.flags & LLMHF_INJECTED) == 0 {
                    STATE.with(|cell| {
                        if let Some(st) = cell.borrow_mut().as_mut() {
                            let pt = hs.pt;
                            let now = Instant::now();

                            if msg == WM_LBUTTONDOWN {
                                if !is_own_window(pt, st.current_pid) {
                                    st.left_down = true;
                                    st.down_pt = pt;
                                    st.down_time = now;
                                    st.down_was_ibeam = st.switches.enable_badge.load(Ordering::Relaxed)
                                        && is_cursor_ibeam(st.ibeam_cursor);
                                    let _ = st.proxy.send_event(UserEvent::ExternalClick);
                                }
                            } else if msg == WM_RBUTTONDOWN || msg == WM_MOUSEWHEEL {
                                if !is_own_window(pt, st.current_pid) {
                                    let _ = st.proxy.send_event(UserEvent::ExternalClick);
                                }
                            } else if msg == WM_LBUTTONUP && st.left_down {
                                st.left_down = false;
                                if st.switches.enable_badge.load(Ordering::Relaxed)
                                    && !is_own_window(pt, st.current_pid)
                                    && (st.down_was_ibeam || is_cursor_ibeam(st.ibeam_cursor))
                                {
                                    let dx = (pt.x - st.down_pt.x) as f64;
                                    let dy = (pt.y - st.down_pt.y) as f64;
                                    let dist = (dx * dx + dy * dy).sqrt();
                                    let drag_ms = now.duration_since(st.down_time).as_millis();

                                    let is_drag = dist >= 14.0 && (40..=15_000).contains(&drag_ms);

                                    let ddx = (pt.x - st.last_up_pt.x) as f64;
                                    let ddy = (pt.y - st.last_up_pt.y) as f64;
                                    let dbl_ms = now.duration_since(st.last_up_time).as_millis();
                                    let is_dbl_click = dist < 8.0
                                        && (ddx * ddx + ddy * ddy).sqrt() < 8.0
                                        && (40..=400).contains(&dbl_ms);

                                    st.last_up_pt = pt;
                                    st.last_up_time = now;

                                    if is_drag || is_dbl_click {
                                        let _ = st
                                            .proxy
                                            .send_event(UserEvent::ShowBadge { x: pt.x, y: pt.y });
                                    }
                                }
                            }
                        }
                    });
                }
            }
        }
        CallNextHookEx(null_mut(), code, wparam, lparam)
    }

    unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let msg = wparam as u32;
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
                if (kb.flags & LLKHF_INJECTED) == 0 {
                    STATE.with(|cell| {
                        if let Some(st) = cell.borrow_mut().as_mut() {
                            let vk = kb.vkCode as u16;
                            if vk == VK_ESCAPE {
                                let _ = st.proxy.send_event(UserEvent::EscapePressed);
                            } else if st.switches.enable_double_copy.load(Ordering::Relaxed)
                                && vk == VK_C
                            {
                                let ctrl = (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
                                let alt = (GetAsyncKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
                                let shift = (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;

                                if ctrl && !alt && !shift {
                                    let now = Instant::now();
                                    if let Some(prev) = st.last_ctrl_c_time {
                                        let elapsed = now.duration_since(prev).as_millis();
                                        if (60..=450).contains(&elapsed) {
                                            st.last_ctrl_c_time = None;
                                            let (cx, cy) = get_cursor_pos_win();
                                            let _ = st
                                                .proxy
                                                .send_event(UserEvent::DoubleCopy { x: cx, y: cy });
                                            return;
                                        }
                                    }
                                    st.last_ctrl_c_time = Some(now);
                                }
                            } else if vk != VK_CONTROL {
                                st.last_ctrl_c_time = None;
                            }
                        }
                    });
                }
            }
        }
        CallNextHookEx(null_mut(), code, wparam, lparam)
    }

    pub fn spawn_win32_hooks(proxy: EventLoopProxy<UserEvent>, switches: Arc<HookSwitches>) {
        std::thread::Builder::new()
            .name("qtvn-win32-hooks".into())
            .spawn(move || unsafe {
                let ibeam = LoadCursorW(null_mut(), IDC_IBEAM) as isize;
                let pid = GetCurrentProcessId();
                let now = Instant::now();

                STATE.with(|cell| {
                    *cell.borrow_mut() = Some(HookState {
                        proxy,
                        switches,
                        current_pid: pid,
                        ibeam_cursor: ibeam,
                        left_down: false,
                        down_pt: POINT { x: 0, y: 0 },
                        down_time: now,
                        down_was_ibeam: false,
                        last_up_pt: POINT { x: 0, y: 0 },
                        last_up_time: now,
                        last_ctrl_c_time: None,
                    });
                });

                let hmod = GetModuleHandleW(null_mut());
                let _m_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hmod, 0);
                let _k_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), hmod, 0);

                let mut msg: MSG = zeroed();
                while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            })
            .ok();
    }
}

// ============================================================================
// MACOS & LINUX HOOKS (`rdev`)
// ============================================================================
#[cfg(not(target_os = "windows"))]
mod unix_hooks {
    use super::*;
    use rdev::{listen, Button, EventType, Key};
    use std::sync::atomic::AtomicI32;
    use std::time::Instant;

    static LAST_X: AtomicI32 = AtomicI32::new(300);
    static LAST_Y: AtomicI32 = AtomicI32::new(300);

    pub fn get_last_cursor_pos() -> (i32, i32) {
        (
            LAST_X.load(Ordering::Relaxed),
            LAST_Y.load(Ordering::Relaxed),
        )
    }

    pub fn spawn_rdev_hooks(proxy: EventLoopProxy<UserEvent>, switches: Arc<HookSwitches>) {
        std::thread::Builder::new()
            .name("qtvn-rdev-hooks".into())
            .spawn(move || {
                let mut left_down = false;
                let mut down_x = 0.0_f64;
                let mut down_y = 0.0_f64;
                let mut down_time = Instant::now();
                let mut last_up_x = 0.0_f64;
                let mut last_up_y = 0.0_f64;
                let mut last_up_time = Instant::now();

                let mut cmd_or_ctrl = false;
                let mut last_copy_time: Option<Instant> = None;

                let _ = listen(move |event| match event.event_type {
                    EventType::MouseMove { x, y } => {
                        LAST_X.store(x as i32, Ordering::Relaxed);
                        LAST_Y.store(y as i32, Ordering::Relaxed);
                    }
                    EventType::ButtonPress(Button::Left) => {
                        left_down = true;
                        down_x = LAST_X.load(Ordering::Relaxed) as f64;
                        down_y = LAST_Y.load(Ordering::Relaxed) as f64;
                        down_time = Instant::now();
                        let _ = proxy.send_event(UserEvent::ExternalClick);
                    }
                    EventType::ButtonPress(Button::Right) | EventType::Wheel { .. } => {
                        let _ = proxy.send_event(UserEvent::ExternalClick);
                    }
                    EventType::ButtonRelease(Button::Left) if left_down => {
                        left_down = false;
                        if switches.enable_badge.load(Ordering::Relaxed) {
                            let ux = LAST_X.load(Ordering::Relaxed) as f64;
                            let uy = LAST_Y.load(Ordering::Relaxed) as f64;
                            let now = Instant::now();
                            let dist = ((ux - down_x).powi(2) + (uy - down_y).powi(2)).sqrt();
                            let drag_ms = now.duration_since(down_time).as_millis();

                            let is_drag = dist >= 16.0 && (50..=15_000).contains(&drag_ms);
                            let dbl_dist =
                                ((ux - last_up_x).powi(2) + (uy - last_up_y).powi(2)).sqrt();
                            let dbl_ms = now.duration_since(last_up_time).as_millis();
                            let is_dbl = dist < 8.0 && dbl_dist < 8.0 && (40..=400).contains(&dbl_ms);

                            last_up_x = ux;
                            last_up_y = uy;
                            last_up_time = now;

                            if is_drag || is_dbl {
                                let _ = proxy.send_event(UserEvent::ShowBadge {
                                    x: ux as i32,
                                    y: uy as i32,
                                });
                            }
                        }
                    }
                    EventType::KeyPress(key) => match key {
                        Key::Escape => {
                            let _ = proxy.send_event(UserEvent::EscapePressed);
                        }
                        Key::ControlLeft
                        | Key::ControlRight
                        | Key::MetaLeft
                        | Key::MetaRight => {
                            cmd_or_ctrl = true;
                        }
                        Key::KeyC if cmd_or_ctrl && switches.enable_double_copy.load(Ordering::Relaxed) => {
                            let now = Instant::now();
                            if let Some(prev) = last_copy_time {
                                let ms = now.duration_since(prev).as_millis();
                                if (60..=450).contains(&ms) {
                                    last_copy_time = None;
                                    let (cx, cy) = get_last_cursor_pos();
                                    let _ = proxy.send_event(UserEvent::DoubleCopy { x: cx, y: cy });
                                    return;
                                }
                            }
                            last_copy_time = Some(now);
                        }
                        _ => {
                            last_copy_time = None;
                        }
                    },
                    EventType::KeyRelease(key) => {
                        if matches!(
                            key,
                            Key::ControlLeft
                                | Key::ControlRight
                                | Key::MetaLeft
                                | Key::MetaRight
                        ) {
                            cmd_or_ctrl = false;
                        }
                    }
                    _ => {}
                });
            })
            .ok();
    }
}
