#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod cleaner;
mod hooks;
mod icon;
mod ocr;
mod settings;
mod shortcut;
mod translator;

use arboard::Clipboard;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use serde::Deserialize;
use single_instance::SingleInstance;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Window, WindowBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::TrayIconBuilder;
use wry::{WebContext, WebViewBuilder};

#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
#[cfg(target_os = "windows")]
use tao::platform::windows::WindowBuilderExtWindows;

use crate::hooks::{
    configure_non_activating_window, get_cursor_physical_pos, start_global_hooks, trim_idle_memory,
    HookSwitches, UserEvent,
};
use crate::settings::AppSettings;
use crate::shortcut::parse_shortcut;
use crate::translator::Translator;

const BADGE_HTML: &str = include_str!("ui_badge.html");
const POPUP_HTML: &str = include_str!("ui_popup.html");
const SHORTCUT_HTML: &str = include_str!("ui_shortcut.html");
#[cfg(target_os = "windows")]
const OCR_OVERLAY_HTML: &str = include_str!("ui_ocr_overlay.html");

const POPUP_WIDTH_DIP: f64 = 384.0;

#[derive(Deserialize)]
struct IpcCmd {
    cmd: String,
    height: Option<f64>,
    pinned: Option<bool>,
    shortcut: Option<String>,
}

fn build_overlay_window(
    event_loop: &tao::event_loop::EventLoopWindowTarget<UserEvent>,
    title: &str,
    width: f64,
    height: f64,
    non_activating: bool,
) -> Window {
    let mut wb = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(width, height))
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_resizable(false)
        .with_focused(!non_activating)
        .with_visible(false);

    #[cfg(target_os = "windows")]
    {
        wb = wb.with_skip_taskbar(true).with_undecorated_shadow(false);
    }

    let win = wb.build(event_loop).expect("failed to build overlay window");
    if non_activating {
        configure_non_activating_window(&win);
    }
    win
}

fn build_shortcut_dialog_window(
    event_loop: &tao::event_loop::EventLoopWindowTarget<UserEvent>,
) -> Window {
    WindowBuilder::new()
        .with_title("QuickTranslateVN — Đặt phím tắt")
        .with_inner_size(LogicalSize::new(340.0, 178.0))
        .with_resizable(false)
        .with_always_on_top(true)
        .with_visible(false)
        .build(event_loop)
        .expect("failed to build shortcut recorder window")
}

fn center_window_on_primary(window: &Window) {
    if let Some(mon) = window.current_monitor().or_else(|| window.primary_monitor()) {
        let m_pos = mon.position();
        let m_size = mon.size();
        let w_size = window.outer_size();
        let x = m_pos.x + ((m_size.width as i32 - w_size.width as i32) / 2).max(20);
        let y = m_pos.y + ((m_size.height as i32 - w_size.height as i32) / 2).max(20);
        window.set_outer_position(PhysicalPosition::new(x, y));
    }
}

#[cfg(target_os = "windows")]
fn cover_active_monitor(window: &Window, cursor: (i32, i32)) {
    let mut target_mon = window.current_monitor().or_else(|| window.primary_monitor());
    for mon in window.available_monitors() {
        let p = mon.position();
        let s = mon.size();
        if cursor.0 >= p.x
            && cursor.0 < p.x + s.width as i32
            && cursor.1 >= p.y
            && cursor.1 < p.y + s.height as i32
        {
            target_mon = Some(mon);
            break;
        }
    }
    if let Some(mon) = target_mon {
        let p = mon.position();
        let s = mon.size();
        window.set_outer_position(PhysicalPosition::new(p.x, p.y));
        window.set_inner_size(PhysicalSize::new(s.width, s.height));
    }
}

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

fn format_shortcut_label(spec: &str) -> String {
    if spec.trim().is_empty() || spec.eq_ignore_ascii_case("None") {
        "Đã tắt".to_string()
    } else {
        spec.replace('+', " + ")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RecorderTarget {
    Selection,
    Ocr,
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

    // 1. Dynamic Cross-Platform Global Shortcuts (Selection `Alt+Q` & Screen OCR `Alt+S`)
    let hotkey_manager = GlobalHotKeyManager::new().ok();
    let active_sel_hk_id = Arc::new(AtomicU32::new(0));
    let active_ocr_hk_id = Arc::new(AtomicU32::new(0));

    let mut active_sel_hk: Option<HotKey> = parse_shortcut(&settings.shortcut);
    let mut active_ocr_hk: Option<HotKey> = parse_shortcut(&settings.ocr_shortcut);

    if let Some(mgr) = &hotkey_manager {
        if let Some(hk) = active_sel_hk {
            if mgr.register(hk).is_ok() {
                active_sel_hk_id.store(hk.id(), Ordering::SeqCst);
            }
        }
        if let Some(hk) = active_ocr_hk {
            if mgr.register(hk).is_ok() {
                active_ocr_hk_id.store(hk.id(), Ordering::SeqCst);
            }
        }
    }

    let hk_proxy = proxy.clone();
    let sel_id_ref = active_sel_hk_id.clone();
    let ocr_id_ref = active_ocr_hk_id.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
        if ev.state == HotKeyState::Pressed {
            let sid = sel_id_ref.load(Ordering::SeqCst);
            let oid = ocr_id_ref.load(Ordering::SeqCst);
            if sid != 0 && ev.id == sid {
                let _ = hk_proxy.send_event(UserEvent::HotkeyTranslate);
            } else if oid != 0 && ev.id == oid {
                let _ = hk_proxy.send_event(UserEvent::HotkeyOcr);
            }
        }
    }));

    // 2. Clean System Tray Menu
    let tray_menu = Menu::new();

    let item_ocr_now = MenuItem::new(
        format!(
            "Chụp vùng màn hình dịch OCR ({})",
            format_shortcut_label(&settings.ocr_shortcut)
        ),
        true,
        None,
    );

    let shortcut_submenu = Submenu::new("Đổi phím tắt nhanh", true);
    let item_change_ocr_sc = MenuItem::new(
        format!(
            "Đặt phím tắt Chụp màn hình OCR (Hiện tại: {})...",
            format_shortcut_label(&settings.ocr_shortcut)
        ),
        true,
        None,
    );
    let item_change_sel_sc = MenuItem::new(
        format!(
            "Đặt phím tắt Dịch chữ bôi đen (Hiện tại: {})...",
            format_shortcut_label(&settings.shortcut)
        ),
        true,
        None,
    );
    let item_reset_sc = MenuItem::new("Khôi phục mặc định (Alt+S & Alt+Q)", true, None);
    let _ = shortcut_submenu.append(&item_change_ocr_sc);
    let _ = shortcut_submenu.append(&item_change_sel_sc);
    let _ = shortcut_submenu.append(&PredefinedMenuItem::separator());
    let _ = shortcut_submenu.append(&item_reset_sc);

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

    let _ = tray_menu.append(&item_ocr_now);
    let _ = tray_menu.append(&shortcut_submenu);
    let _ = tray_menu.append(&PredefinedMenuItem::separator());
    let _ = tray_menu.append(&item_badge);
    let _ = tray_menu.append(&item_dbl_copy);
    let _ = tray_menu.append(&item_startup);
    let _ = tray_menu.append(&PredefinedMenuItem::separator());
    let _ = tray_menu.append(&item_quit);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip(format!(
            "QuickTranslateVN — OCR: {} | Dịch chọn: {}",
            format_shortcut_label(&settings.ocr_shortcut),
            format_shortcut_label(&settings.shortcut)
        ))
        .with_icon(icon::build_tray_icon())
        .build()
        .ok();

    let id_ocr_now = item_ocr_now.id().clone();
    let id_change_ocr_sc = item_change_ocr_sc.id().clone();
    let id_change_sel_sc = item_change_sel_sc.id().clone();
    let id_reset_sc = item_reset_sc.id().clone();
    let id_badge = item_badge.id().clone();
    let id_dbl_copy = item_dbl_copy.id().clone();
    let id_startup = item_startup.id().clone();
    let id_quit = item_quit.id().clone();

    // 3. Shared WebContext & Pre-warmed Windows
    let webview_data_dir = dirs::data_local_dir().map(|mut p| {
        p.push("QuickTranslateVN");
        p.push("WebViewCache");
        p
    });
    let mut web_context = WebContext::new(webview_data_dir);

    let badge_win = build_overlay_window(&event_loop, "QuickTranslateVN_Badge", 38.0, 38.0, true);
    let badge_ipc_proxy = proxy.clone();
    let _badge_webview = WebViewBuilder::with_web_context(&mut web_context)
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
        true,
    );
    let popup_ipc_proxy = proxy.clone();
    let popup_webview = WebViewBuilder::with_web_context(&mut web_context)
        .with_transparent(true)
        .with_html(POPUP_HTML)
        .with_ipc_handler(move |req| {
            let _ = popup_ipc_proxy.send_event(UserEvent::IpcMessage(req.body().to_string()));
        })
        .build(&popup_win)
        .expect("failed to build popup webview");

    #[cfg(target_os = "windows")]
    let (ocr_win, ocr_webview) = {
        let ow = build_overlay_window(&event_loop, "QuickTranslateVN_OcrSnip", 800.0, 600.0, false);
        let op = proxy.clone();
        let wv = WebViewBuilder::with_web_context(&mut web_context)
            .with_transparent(true)
            .with_html(OCR_OVERLAY_HTML)
            .with_ipc_handler(move |req| {
                let _ = op.send_event(UserEvent::IpcMessage(req.body().to_string()));
            })
            .build(&ow)
            .expect("failed to build OCR snipping webview");
        (ow, wv)
    };

    let shortcut_win = build_shortcut_dialog_window(&event_loop);
    let shortcut_win_id = shortcut_win.id();
    let shortcut_ipc_proxy = proxy.clone();
    let shortcut_webview = WebViewBuilder::with_web_context(&mut web_context)
        .with_html(SHORTCUT_HTML)
        .with_ipc_handler(move |req| {
            let _ = shortcut_ipc_proxy.send_event(UserEvent::IpcMessage(req.body().to_string()));
        })
        .build(&shortcut_win)
        .expect("failed to build shortcut recorder webview");

    // 4. Global Input Hooks
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
    let mut recorder_target = RecorderTarget::Ocr;
    #[cfg(target_os = "windows")]
    let mut ocr_start_pt: (i32, i32) = (0, 0);

    let mut current_original = String::new();
    let mut current_translated = String::new();
    let mut current_src_lang = String::from("en");
    let mut current_tgt_lang = String::from("vi");

    trim_idle_memory();

    let refresh_hotkeys_and_labels = |settings: &AppSettings,
                                      mgr_opt: &Option<GlobalHotKeyManager>,
                                      sel_hk: &mut Option<HotKey>,
                                      ocr_hk: &mut Option<HotKey>,
                                      sel_id: &Arc<AtomicU32>,
                                      ocr_id: &Arc<AtomicU32>,
                                      ocr_now_item: &MenuItem,
                                      chg_ocr_item: &MenuItem,
                                      chg_sel_item: &MenuItem,
                                      tray: &Option<tray_icon::TrayIcon>| {
        if let Some(mgr) = mgr_opt {
            if let Some(old) = *sel_hk {
                let _ = mgr.unregister(old);
            }
            if let Some(old) = *ocr_hk {
                let _ = mgr.unregister(old);
            }
        }
        *sel_hk = parse_shortcut(&settings.shortcut);
        *ocr_hk = parse_shortcut(&settings.ocr_shortcut);
        sel_id.store(0, Ordering::SeqCst);
        ocr_id.store(0, Ordering::SeqCst);

        if let Some(mgr) = mgr_opt {
            if let Some(hk) = *sel_hk {
                if mgr.register(hk).is_ok() {
                    sel_id.store(hk.id(), Ordering::SeqCst);
                }
            }
            if let Some(hk) = *ocr_hk {
                if mgr.register(hk).is_ok() {
                    ocr_id.store(hk.id(), Ordering::SeqCst);
                }
            }
        }

        let ocr_lbl = format_shortcut_label(&settings.ocr_shortcut);
        let sel_lbl = format_shortcut_label(&settings.shortcut);

        ocr_now_item.set_text(format!("Chụp vùng màn hình dịch OCR ({})", ocr_lbl));
        chg_ocr_item.set_text(format!(
            "Đặt phím tắt Chụp màn hình OCR (Hiện tại: {})...",
            ocr_lbl
        ));
        chg_sel_item.set_text(format!(
            "Đặt phím tắt Dịch chữ bôi đen (Hiện tại: {})...",
            sel_lbl
        ));
        if let Some(t) = tray {
            let _ = t.set_tooltip(Some(format!(
                "QuickTranslateVN — OCR: {} | Dịch chọn: {}",
                ocr_lbl, sel_lbl
            )));
        }
    };

    event_loop.run(move |event, _, control_flow| {
        let _ = (&web_context, &tray_icon, &hotkey_manager);
        if let Some(deadline) = badge_hide_deadline {
            *control_flow = ControlFlow::WaitUntil(deadline);
        } else {
            *control_flow = ControlFlow::Wait;
        }

        if let Ok(menu_ev) = MenuEvent::receiver().try_recv() {
            if menu_ev.id == id_ocr_now {
                let _ = proxy.send_event(UserEvent::HotkeyOcr);
            } else if menu_ev.id == id_change_ocr_sc || menu_ev.id == id_change_sel_sc {
                recorder_target = if menu_ev.id == id_change_ocr_sc {
                    RecorderTarget::Ocr
                } else {
                    RecorderTarget::Selection
                };

                // Suspend hotkeys while recording
                active_sel_hk_id.store(0, Ordering::SeqCst);
                active_ocr_hk_id.store(0, Ordering::SeqCst);
                if let Some(mgr) = &hotkey_manager {
                    if let Some(hk) = active_sel_hk {
                        let _ = mgr.unregister(hk);
                    }
                    if let Some(hk) = active_ocr_hk {
                        let _ = mgr.unregister(hk);
                    }
                }

                let cur_spec = match recorder_target {
                    RecorderTarget::Ocr => &settings.ocr_shortcut,
                    RecorderTarget::Selection => &settings.shortcut,
                };
                if let Ok(js_str) = serde_json::to_string(&format_shortcut_label(cur_spec)) {
                    let _ = shortcut_webview
                        .evaluate_script(&format!("window.__setInitialShortcut({});", js_str));
                }
                center_window_on_primary(&shortcut_win);
                shortcut_win.set_visible(true);
                shortcut_win.set_focus();
            } else if menu_ev.id == id_reset_sc {
                settings.ocr_shortcut = "Alt+S".to_string();
                settings.shortcut = "Alt+Q".to_string();
                settings.save();
                refresh_hotkeys_and_labels(
                    &settings,
                    &hotkey_manager,
                    &mut active_sel_hk,
                    &mut active_ocr_hk,
                    &active_sel_hk_id,
                    &active_ocr_hk_id,
                    &item_ocr_now,
                    &item_change_ocr_sc,
                    &item_change_sel_sc,
                    &tray_icon,
                );
            } else if menu_ev.id == id_badge {
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
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } if window_id == shortcut_win_id => {
                shortcut_win.set_visible(false);
                refresh_hotkeys_and_labels(
                    &settings,
                    &hotkey_manager,
                    &mut active_sel_hk,
                    &mut active_ocr_hk,
                    &active_sel_hk_id,
                    &active_ocr_hk_id,
                    &item_ocr_now,
                    &item_change_ocr_sc,
                    &item_change_sel_sc,
                    &tray_icon,
                );
            }

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
                    #[cfg(target_os = "windows")]
                    let ocr_open = ocr_win.is_visible();
                    #[cfg(not(target_os = "windows"))]
                    let ocr_open = false;

                    if !busy_capturing.load(Ordering::Relaxed)
                        && !shortcut_win.is_visible()
                        && !ocr_open
                    {
                        badge_anchor = (x, y);
                        position_window_near(&badge_win, x, y, 8.0, 10.0);
                        badge_win.set_visible(true);
                        badge_hide_deadline = Some(Instant::now() + Duration::from_millis(3800));
                    }
                }

                UserEvent::BadgeClicked => {
                    badge_win.set_visible(false);
                    badge_hide_deadline = None;
                    spawn_capture_worker(
                        badge_anchor,
                        false,
                        settings.clean_pdf_line_breaks,
                        &req_counter,
                        &busy_capturing,
                        &proxy,
                    );
                }

                UserEvent::HotkeyTranslate => {
                    if shortcut_win.is_visible() {
                        return;
                    }
                    badge_win.set_visible(false);
                    badge_hide_deadline = None;
                    let pt = get_cursor_physical_pos();
                    spawn_capture_worker(
                        pt,
                        false,
                        settings.clean_pdf_line_breaks,
                        &req_counter,
                        &busy_capturing,
                        &proxy,
                    );
                }

                UserEvent::HotkeyOcr => {
                    if shortcut_win.is_visible() || busy_capturing.load(Ordering::Relaxed) {
                        return;
                    }
                    badge_win.set_visible(false);
                    badge_hide_deadline = None;
                    if !is_pinned && popup_win.is_visible() {
                        popup_win.set_visible(false);
                    }

                    #[cfg(target_os = "windows")]
                    {
                        let cur = get_cursor_physical_pos();
                        let _ = ocr_webview.evaluate_script("window.__resetOverlay();");
                        cover_active_monitor(&ocr_win, cur);
                        ocr_win.set_visible(true);
                        ocr_win.set_focus();
                    }

                    #[cfg(not(target_os = "windows"))]
                    {
                        let rid = req_counter.fetch_add(1, Ordering::SeqCst) + 1;
                        let prx = proxy.clone();
                        let clean_pdf = settings.clean_pdf_line_breaks;
                        std::thread::spawn(move || match ocr::interactive_capture_and_ocr() {
                            Ok(raw) => {
                                let cleaned = cleaner::clean_text(&raw, clean_pdf);
                                if !cleaned.is_empty() {
                                    let anchor = get_cursor_physical_pos();
                                    let _ = prx.send_event(UserEvent::TextCaptured {
                                        req_id: rid,
                                        anchor,
                                        cleaned_text: cleaned,
                                    });
                                }
                            }
                            Err(msg) if !msg.is_empty() => {
                                let anchor = get_cursor_physical_pos();
                                let _ = prx.send_event(UserEvent::OcrFailed {
                                    anchor,
                                    error_msg: msg,
                                });
                            }
                            _ => {}
                        });
                    }
                }

                UserEvent::DoubleCopy { x, y } => {
                    if shortcut_win.is_visible() {
                        return;
                    }
                    badge_win.set_visible(false);
                    badge_hide_deadline = None;
                    spawn_capture_worker(
                        (x, y),
                        true,
                        settings.clean_pdf_line_breaks,
                        &req_counter,
                        &busy_capturing,
                        &proxy,
                    );
                }

                UserEvent::TextCaptured {
                    req_id,
                    anchor,
                    cleaned_text,
                } => {
                    popup_anchor = anchor;
                    let _ = popup_webview.evaluate_script("window.__renderLoading();");
                    popup_win.set_inner_size(LogicalSize::new(POPUP_WIDTH_DIP, 88.0));
                    position_window_near(&popup_win, anchor.0, anchor.1, 8.0, 14.0);
                    popup_win.set_visible(true);

                    let tr = translator.clone();
                    let prx = proxy.clone();
                    std::thread::spawn(move || {
                        let payload = match tr.translate(&cleaned_text, None) {
                            Ok(r) => serde_json::to_string(&r).unwrap_or_default(),
                            Err(e) => e,
                        };
                        let _ = prx.send_event(UserEvent::TranslationCompleted {
                            req_id,
                            payload_json: payload,
                        });
                    });
                }

                UserEvent::OcrFailed { anchor, error_msg } => {
                    popup_anchor = anchor;
                    if let Ok(quoted) = serde_json::to_string(&error_msg) {
                        let _ = popup_webview
                            .evaluate_script(&format!("window.__renderError({});", quoted));
                    }
                    popup_win.set_inner_size(LogicalSize::new(POPUP_WIDTH_DIP, 92.0));
                    position_window_near(&popup_win, anchor.0, anchor.1, 8.0, 14.0);
                    popup_win.set_visible(true);
                }

                UserEvent::ExternalClick => {
                    badge_win.set_visible(false);
                    badge_hide_deadline = None;
                    if !is_pinned && popup_win.is_visible() {
                        popup_win.set_visible(false);
                        trim_idle_memory();
                    }
                }

                UserEvent::EscapePressed => {
                    #[cfg(target_os = "windows")]
                    if ocr_win.is_visible() {
                        ocr_win.set_visible(false);
                        trim_idle_memory();
                        return;
                    }
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
                            #[cfg(target_os = "windows")]
                            "ocr_drag_start" => {
                                ocr_start_pt = get_cursor_physical_pos();
                            }
                            #[cfg(target_os = "windows")]
                            "ocr_drag_end" => {
                                let end_pt = get_cursor_physical_pos();
                                ocr_win.set_visible(false);

                                let rx = ocr_start_pt.0.min(end_pt.0);
                                let ry = ocr_start_pt.1.min(end_pt.1);
                                let rw = (end_pt.0 - ocr_start_pt.0).abs();
                                let rh = (end_pt.1 - ocr_start_pt.1).abs();

                                if rw >= 10 && rh >= 10 {
                                    let rid = req_counter.fetch_add(1, Ordering::SeqCst) + 1;
                                    let prx = proxy.clone();
                                    let clean_pdf = settings.clean_pdf_line_breaks;
                                    std::thread::spawn(move || {
                                        // Give compositor 40ms to clear the dim overlay before BitBlt
                                        std::thread::sleep(Duration::from_millis(40));
                                        match ocr::capture_rect_and_ocr(rx, ry, rw, rh) {
                                            Ok(raw_ocr) => {
                                                let cleaned =
                                                    cleaner::clean_text(&raw_ocr, clean_pdf);
                                                if !cleaned.is_empty() {
                                                    let _ =
                                                        prx.send_event(UserEvent::TextCaptured {
                                                            req_id: rid,
                                                            anchor: end_pt,
                                                            cleaned_text: cleaned,
                                                        });
                                                }
                                            }
                                            Err(err_msg) => {
                                                let _ = prx.send_event(UserEvent::OcrFailed {
                                                    anchor: end_pt,
                                                    error_msg: err_msg,
                                                });
                                            }
                                        }
                                    });
                                } else {
                                    trim_idle_memory();
                                }
                            }
                            #[cfg(target_os = "windows")]
                            "ocr_cancel" => {
                                ocr_win.set_visible(false);
                                trim_idle_memory();
                            }
                            "shortcut_save" => {
                                shortcut_win.set_visible(false);
                                if let Some(sc) = msg.shortcut {
                                    match recorder_target {
                                        RecorderTarget::Ocr => settings.ocr_shortcut = sc,
                                        RecorderTarget::Selection => settings.shortcut = sc,
                                    }
                                    settings.save();
                                }
                                refresh_hotkeys_and_labels(
                                    &settings,
                                    &hotkey_manager,
                                    &mut active_sel_hk,
                                    &mut active_ocr_hk,
                                    &active_sel_hk_id,
                                    &active_ocr_hk_id,
                                    &item_ocr_now,
                                    &item_change_ocr_sc,
                                    &item_change_sel_sc,
                                    &tray_icon,
                                );
                            }
                            "shortcut_cancel" => {
                                shortcut_win.set_visible(false);
                                refresh_hotkeys_and_labels(
                                    &settings,
                                    &hotkey_manager,
                                    &mut active_sel_hk,
                                    &mut active_ocr_hk,
                                    &active_sel_hk_id,
                                    &active_ocr_hk_id,
                                    &item_ocr_now,
                                    &item_change_ocr_sc,
                                    &item_change_sel_sc,
                                    &tray_icon,
                                );
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

fn spawn_capture_worker(
    anchor: (i32, i32),
    from_double_copy: bool,
    clean_pdf: bool,
    req_counter: &Arc<AtomicU64>,
    busy_capturing: &Arc<AtomicBool>,
    proxy: &tao::event_loop::EventLoopProxy<UserEvent>,
) {
    if busy_capturing.swap(true, Ordering::SeqCst) {
        return;
    }

    let rid = req_counter.fetch_add(1, Ordering::SeqCst) + 1;
    let prx = proxy.clone();
    let busy = busy_capturing.clone();

    std::thread::spawn(move || {
        let raw_opt = if from_double_copy {
            capture::read_current_clipboard()
        } else {
            capture::capture_selected_text()
        };
        busy.store(false, Ordering::SeqCst);

        if let Some(raw_text) = raw_opt {
            let cleaned = cleaner::clean_text(&raw_text, clean_pdf);
            if !cleaned.is_empty() {
                let _ = prx.send_event(UserEvent::TextCaptured {
                    req_id: rid,
                    anchor,
                    cleaned_text: cleaned,
                });
            }
        }
    });
}
