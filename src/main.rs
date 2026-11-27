#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod cleaner;
mod hooks;
mod icon;
mod settings;
mod translator;

use arboard::Clipboard;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use serde::Deserialize;
use single_instance::SingleInstance;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Window, WindowBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::TrayIconBuilder;
use wry::WebViewBuilder;

#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
#[cfg(target_os = "windows")]
use tao::platform::windows::WindowBuilderExtWindows;

use crate::hooks::{
    configure_non_activating_window, get_cursor_physical_pos, start_global_hooks, trim_idle_memory,
    HookSwitches, UserEvent,
};
use crate::settings::AppSettings;
use crate::translator::Translator;

const BADGE_HTML: &str = include_str!("ui_badge.html");
const POPUP_HTML: &str = include_str!("ui_popup.html");
const POPUP_WIDTH_DIP: f64 = 384.0;

#[derive(Deserialize)]
struct IpcCmd {
    cmd: String,
    height: Option<f64>,
    pinned: Option<bool>,
}

fn build_overlay_window(
    event_loop: &tao::event_loop::EventLoopWindowTarget<UserEvent>,
    title: &str,
    width: f64,
    height: f64,
) -> Window {
    let mut wb = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(width, height))
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_resizable(false)
        .with_focused(false)
        .with_visible(false);

    #[cfg(target_os = "windows")]
    {
        wb = wb.with_skip_taskbar(true).with_undecorated_shadow(false);
    }

    let win = wb.build(event_loop).expect("failed to build overlay window");
    configure_non_activating_window(&win);
    win
}

/// Positions `window` cleanly near physical cursor `(ax, ay)` while clamping to the active monitor bounds.
fn position_window_near(window: &Window, ax: i32, ay: i32, offset_dip_x: f64, offset_dip_y: f64) {
    let mut target_monitor = window.current_monitor().or_else(|| window.primary_monitor());
    for mon in window.available_monitors() {
        let pos = mon.position();
        let size = mon.size();
        if ax >= pos.x
            && ax < pos.x + size.width as i32
            && ay >= pos.y
            && ay < pos.y + size.height as i32
        {
            target_monitor = Some(mon);
            break;
        }
    }

    let scale = target_monitor
        .as_ref()
        .map(|m| m.scale_factor())
        .unwrap_or_else(|| window.scale_factor());

    let win_size: PhysicalSize<u32> = window.outer_size();
    let win_w = win_size.width as i32;
    let win_h = win_size.height as i32;

    let mut px = ax + (offset_dip_x * scale) as i32;
    let mut py = ay + (offset_dip_y * scale) as i32;

    if let Some(mon) = target_monitor {
        let m_pos = mon.position();
        let m_size = mon.size();
        let margin = (12.0 * scale) as i32;
        let taskbar_pad = (46.0 * scale) as i32;

        let left_bound = m_pos.x + margin;
        let top_bound = m_pos.y + margin;
        let right_bound = m_pos.x + m_size.width as i32 - margin;
        let bottom_bound = m_pos.y + m_size.height as i32 - taskbar_pad;

        if px + win_w > right_bound {
            px = (right_bound - win_w).max(left_bound);
        }
        if px < left_bound {
            px = left_bound;
        }
        if py + win_h > bottom_bound {
            let flipped = ay - win_h - (14.0 * scale) as i32;
            py = if flipped >= top_bound {
                flipped
            } else {
                (bottom_bound - win_h).max(top_bound)
            };
        }
    }

    window.set_outer_position(PhysicalPosition::new(px, py));
}

fn main() {
    let instance = SingleInstance::new("quick-translate-vn-rust-instance").unwrap();
    if !instance.is_single() {
        return;
    }

    let mut settings = AppSettings::load();
    let mut event_loop_builder = EventLoopBuilder::<UserEvent>::with_user_event();
    #[allow(unused_mut)]
    let mut event_loop = event_loop_builder.build();

    #[cfg(target_os = "macos")]
    {
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }

    let proxy = event_loop.create_proxy();

    // 1. Cross-platform Global Hotkey: Alt + Q (Option + Q on macOS)
    let hotkey_manager = GlobalHotKeyManager::new().ok();
    let hk_alt_q = HotKey::new(Some(Modifiers::ALT), Code::KeyQ);
    if let Some(mgr) = &hotkey_manager {
        let _ = mgr.register(hk_alt_q);
    }

    let hk_proxy = proxy.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
        if ev.state == HotKeyState::Pressed && ev.id == hk_alt_q.id() {
            let _ = hk_proxy.send_event(UserEvent::HotkeyTranslate);
        }
    }));

    // 2. System Tray Menu (Windows System Tray / macOS Menu Bar / Linux StatusNotifier)
    let tray_menu = Menu::new();
    let item_badge = CheckMenuItem::new(
        "Hiện nút [文A] khi bôi đen chuột",
        true,
        settings.show_floating_badge,
        None,
    );
    let item_dbl_copy = CheckMenuItem::new(
        "Dịch khi nhấn đúp Ctrl/Cmd + C + C",
        true,
        settings.enable_double_copy,
        None,
    );
    let item_startup = CheckMenuItem::new(
        "Khởi động cùng hệ thống",
        true,
        AppSettings::is_startup_enabled(),
        None,
    );
    let item_quit = MenuItem::new("Thoát", true, None);

    let _ = tray_menu.append(&item_badge);
    let _ = tray_menu.append(&item_dbl_copy);
    let _ = tray_menu.append(&item_startup);
    let _ = tray_menu.append(&PredefinedMenuItem::separator());
    let _ = tray_menu.append(&item_quit);

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("QuickTranslateVN (Bôi đen chữ hoặc nhấn Alt+Q / Ctrl+C+C)")
        .with_icon(icon::build_tray_icon())
        .build()
        .ok();

    let id_badge = item_badge.id().clone();
    let id_dbl_copy = item_dbl_copy.id().clone();
    let id_startup = item_startup.id().clone();
    let id_quit = item_quit.id().clone();

    // 3. Pre-warmed Floating Badge & Clean White Popup Windows + WebViews
    let badge_win = build_overlay_window(&event_loop, "QuickTranslateVN_Badge", 38.0, 38.0);
    let badge_ipc_proxy = proxy.clone();
    let _badge_webview = WebViewBuilder::new()
        .with_transparent(true)
        .with_html(BADGE_HTML)
        .with_ipc_handler(move |req| {
            let _ = badge_ipc_proxy.send_event(UserEvent::IpcMessage(req.body().to_string()));
        })
        .build(&badge_win)
        .expect("failed to build badge webview");

    let popup_win = build_overlay_window(
        &event_loop,
        "QuickTranslateVN_Popup",
        POPUP_WIDTH_DIP,
        110.0,
    );
    let popup_ipc_proxy = proxy.clone();
    let popup_webview = WebViewBuilder::new()
        .with_transparent(true)
        .with_html(POPUP_HTML)
        .with_ipc_handler(move |req| {
            let _ = popup_ipc_proxy.send_event(UserEvent::IpcMessage(req.body().to_string()));
        })
        .build(&popup_win)
        .expect("failed to build popup webview");

    // 4. Global Input Hooks (Mouse Selection & Double-Copy & ESC)
    let switches = Arc::new(HookSwitches {
        enable_badge: AtomicBool::new(settings.show_floating_badge),
        enable_double_copy: AtomicBool::new(settings.enable_double_copy),
    });
    start_global_hooks(proxy.clone(), switches.clone());

    let translator = Arc::new(Translator::new());
    let req_counter = Arc::new(AtomicU64::new(0));
    let busy_capturing = Arc::new(AtomicBool::new(false));

    let mut badge_anchor: (i32, i32) = (200, 200);
    let mut badge_hide_deadline: Option<Instant> = None;
    let mut popup_anchor: (i32, i32) = (200, 200);
    let mut is_pinned = false;

    let mut current_original = String::new();
    let mut current_translated = String::new();
    let mut current_src_lang = String::from("en");
    let mut current_tgt_lang = String::from("vi");

    trim_idle_memory();

    event_loop.run(move |event, _, control_flow| {
        if let Some(deadline) = badge_hide_deadline {
            *control_flow = ControlFlow::WaitUntil(deadline);
        } else {
            *control_flow = ControlFlow::Wait;
        }

        // Handle Tray Menu Clicks
        if let Ok(menu_ev) = MenuEvent::receiver().try_recv() {
            if menu_ev.id == id_badge {
                settings.show_floating_badge = item_badge.is_checked();
                switches
                    .enable_badge
                    .store(settings.show_floating_badge, Ordering::Relaxed);
                settings.save();
            } else if menu_ev.id == id_dbl_copy {
                settings.enable_double_copy = item_dbl_copy.is_checked();
                switches
                    .enable_double_copy
                    .store(settings.enable_double_copy, Ordering::Relaxed);
                settings.save();
            } else if menu_ev.id == id_startup {
                AppSettings::set_startup_enabled(item_startup.is_checked());
            } else if menu_ev.id == id_quit {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        match event {
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                if let Some(dl) = badge_hide_deadline {
                    if Instant::now() >= dl {
                        badge_win.set_visible(false);
                        badge_hide_deadline = None;
                    }
                }
            }

            Event::UserEvent(ue) => match ue {
                UserEvent::ShowBadge { x, y } => {
                    if !busy_capturing.load(Ordering::Relaxed) {
                        badge_anchor = (x, y);
                        position_window_near(&badge_win, x, y, 8.0, 10.0);
                        badge_win.set_visible(true);
                        badge_hide_deadline = Some(Instant::now() + Duration::from_millis(3800));
                    }
                }

                UserEvent::BadgeClicked => {
                    badge_win.set_visible(false);
                    badge_hide_deadline = None;
                    trigger_capture_and_translate(
                        badge_anchor,
                        false,
                        settings.clean_pdf_line_breaks,
                        &popup_win,
                        &popup_webview,
                        &mut popup_anchor,
                        &translator,
                        &req_counter,
                        &busy_capturing,
                        &proxy,
                    );
                }

                UserEvent::HotkeyTranslate => {
                    badge_win.set_visible(false);
                    badge_hide_deadline = None;
                    let pt = get_cursor_physical_pos();
                    trigger_capture_and_translate(
                        pt,
                        false,
                        settings.clean_pdf_line_breaks,
                        &popup_win,
                        &popup_webview,
                        &mut popup_anchor,
                        &translator,
                        &req_counter,
                        &busy_capturing,
                        &proxy,
                    );
                }

                UserEvent::DoubleCopy { x, y } => {
                    badge_win.set_visible(false);
                    badge_hide_deadline = None;
                    trigger_capture_and_translate(
                        (x, y),
                        true,
                        settings.clean_pdf_line_breaks,
                        &popup_win,
                        &popup_webview,
                        &mut popup_anchor,
                        &translator,
                        &req_counter,
                        &busy_capturing,
                        &proxy,
                    );
                }

                UserEvent::ExternalClick | UserEvent::EscapePressed => {
                    badge_win.set_visible(false);
                    badge_hide_deadline = None;
                    if !is_pinned && popup_win.is_visible() {
                        popup_win.set_visible(false);
                        trim_idle_memory();
                    }
                }

                UserEvent::TranslationCompleted {
                    req_id,
                    payload_json,
                } => {
                    if req_id == req_counter.load(Ordering::SeqCst) {
                        if let Ok(parsed) =
                            serde_json::from_str::<translator::TranslationResult>(&payload_json)
                        {
                            current_original = parsed.original_text.clone();
                            current_translated = parsed.translated_text.clone();
                            current_src_lang = parsed.source_lang.clone();
                            current_tgt_lang = parsed.target_lang.clone();
                            let script = format!("window.__renderResult({});", payload_json);
                            let _ = popup_webview.evaluate_script(&script);
                        } else {
                            let script = format!(
                                "window.__renderError({});",
                                serde_json::to_string(&payload_json).unwrap_or_default()
                            );
                            let _ = popup_webview.evaluate_script(&script);
                        }
                    }
                }

                UserEvent::TtsAudioReady { data_uri } => {
                    if popup_win.is_visible() {
                        if let Ok(quoted) = serde_json::to_string(&data_uri) {
                            let _ = popup_webview
                                .evaluate_script(&format!("window.__playAudio({});", quoted));
                        }
                    }
                }

                UserEvent::IpcMessage(raw) => {
                    if let Ok(msg) = serde_json::from_str::<IpcCmd>(&raw) {
                        match msg.cmd.as_str() {
                            "badge_clicked" => {
                                let _ = proxy.send_event(UserEvent::BadgeClicked);
                            }
                            "drag_window" => {
                                let _ = popup_win.drag_window();
                            }
                            "resize" => {
                                if let Some(h) = msg.height {
                                    popup_win.set_inner_size(LogicalSize::new(POPUP_WIDTH_DIP, h));
                                    if !is_pinned {
                                        position_window_near(
                                            &popup_win,
                                            popup_anchor.0,
                                            popup_anchor.1,
                                            8.0,
                                            14.0,
                                        );
                                    }
                                }
                            }
                            "swap_lang" => {
                                if !current_original.is_empty() {
                                    let next_target = if current_tgt_lang == "vi" {
                                        "en"
                                    } else {
                                        "vi"
                                    };
                                    let _ =
                                        popup_webview.evaluate_script("window.__renderLoading();");
                                    let rid = req_counter.fetch_add(1, Ordering::SeqCst) + 1;
                                    let tr = translator.clone();
                                    let orig = current_original.clone();
                                    let tgt = next_target.to_string();
                                    let prx = proxy.clone();
                                    std::thread::spawn(move || {
                                        let payload = match tr.translate(&orig, Some(&tgt)) {
                                            Ok(r) => serde_json::to_string(&r).unwrap_or_default(),
                                            Err(e) => e,
                                        };
                                        let _ = prx.send_event(UserEvent::TranslationCompleted {
                                            req_id: rid,
                                            payload_json: payload,
                                        });
                                    });
                                }
                            }
                            "speak" => {
                                if !current_original.is_empty() {
                                    let tr = translator.clone();
                                    let orig = current_original.clone();
                                    let lang = current_src_lang.clone();
                                    let prx = proxy.clone();
                                    std::thread::spawn(move || {
                                        if let Ok(uri) = tr.fetch_tts_data_uri(&orig, &lang) {
                                            let _ =
                                                prx.send_event(UserEvent::TtsAudioReady {
                                                    data_uri: uri,
                                                });
                                        }
                                    });
                                }
                            }
                            "copy" => {
                                if !current_translated.is_empty() {
                                    if let Ok(mut cb) = Clipboard::new() {
                                        let _ = cb.set_text(current_translated.clone());
                                    }
                                }
                            }
                            "replace" => {
                                if !current_translated.is_empty() {
                                    let to_insert = current_translated.clone();
                                    popup_win.set_visible(false);
                                    std::thread::spawn(move || {
                                        std::thread::sleep(Duration::from_millis(30));
                                        let _ = capture::replace_selected_text(&to_insert);
                                    });
                                }
                            }
                            "pin" => {
                                is_pinned = msg.pinned.unwrap_or(false);
                            }
                            "close" => {
                                popup_win.set_visible(false);
                                trim_idle_memory();
                            }
                            _ => {}
                        }
                    }
                }
            },
            _ => {}
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn trigger_capture_and_translate(
    anchor: (i32, i32),
    from_double_copy: bool,
    clean_pdf: bool,
    popup_win: &Window,
    popup_webview: &wry::WebView,
    popup_anchor: &mut (i32, i32),
    translator: &Arc<Translator>,
    req_counter: &Arc<AtomicU64>,
    busy_capturing: &Arc<AtomicBool>,
    proxy: &tao::event_loop::EventLoopProxy<UserEvent>,
) {
    if busy_capturing.swap(true, Ordering::SeqCst) {
        return;
    }

    *popup_anchor = anchor;
    let rid = req_counter.fetch_add(1, Ordering::SeqCst) + 1;

    // Show clean loading popup only once text is confirmed captured so empty clicks don't flash UI
    let tr = translator.clone();
    let prx = proxy.clone();
    let busy = busy_capturing.clone();

    let raw_opt = if from_double_copy {
        capture::read_current_clipboard()
    } else {
        capture::capture_selected_text()
    };

    let Some(raw_text) = raw_opt else {
        busy.store(false, Ordering::SeqCst);
        return;
    };

    let cleaned = cleaner::clean_text(&raw_text, clean_pdf);
    if cleaned.is_empty() {
        busy.store(false, Ordering::SeqCst);
        return;
    }

    let _ = popup_webview.evaluate_script("window.__renderLoading();");
    popup_win.set_inner_size(LogicalSize::new(POPUP_WIDTH_DIP, 88.0));
    position_window_near(popup_win, anchor.0, anchor.1, 8.0, 14.0);
    popup_win.set_visible(true);

    std::thread::spawn(move || {
        let payload = match tr.translate(&cleaned, None) {
            Ok(r) => serde_json::to_string(&r).unwrap_or_default(),
            Err(e) => e,
        };
        busy.store(false, Ordering::SeqCst);
        let _ = prx.send_event(UserEvent::TranslationCompleted {
            req_id: rid,
            payload_json: payload,
        });
    });
}
