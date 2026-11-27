#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod cleaner;
mod hooks;
mod icon;
mod ocr;
mod platform;
mod settings;
mod shortcut;
mod translator;
mod tray;
mod window_ext;

use arboard::Clipboard;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tao::dpi::LogicalSize;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tray_icon::menu::MenuEvent;
use wry::{WebContext, WebViewBuilder};

#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};

use crate::hooks::{
    get_cursor_physical_pos, start_global_hooks, trim_idle_memory, HookSwitches, UserEvent,
};
use crate::platform::{app_cache_dir, SingleInstanceGuard};
use crate::settings::AppSettings;
use crate::shortcut::format_shortcut_label;
use crate::translator::Translator;
use crate::tray::{TrayAction, TrayController};
use crate::window_ext::{
    build_overlay_window, build_shortcut_dialog_window, center_window_on_primary,
    position_window_near, POPUP_WIDTH_DIP,
};

const BADGE_HTML: &str = include_str!("ui_badge.html");
const POPUP_HTML: &str = include_str!("ui_popup.html");
const SHORTCUT_HTML: &str = include_str!("ui_shortcut.html");
#[cfg(target_os = "windows")]
const OCR_OVERLAY_HTML: &str = include_str!("ui_ocr_overlay.html");

#[derive(Deserialize)]
struct IpcCmd {
    cmd: String,
    height: Option<f64>,
    pinned: Option<bool>,
    shortcut: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RecorderTarget {
    Selection,
    Ocr,
}

#[derive(Default)]
struct AppState {
    badge_anchor: (i32, i32),
    badge_hide_deadline: Option<Instant>,
    popup_anchor: (i32, i32),
    is_pinned: bool,
    recorder_target: Option<RecorderTarget>,
    #[cfg(target_os = "windows")]
    ocr_start_pt: (i32, i32),
    original_text: String,
    translated_text: String,
    source_lang: String,
    target_lang: String,
}

fn main() {
    let Some(_instance_guard) = SingleInstanceGuard::try_acquire() else {
        return;
    };

    let mut settings = AppSettings::load();
    let mut event_loop_builder = EventLoopBuilder::<UserEvent>::with_user_event();
    #[allow(unused_mut)]
    let mut event_loop = event_loop_builder.build();

    #[cfg(target_os = "macos")]
    {
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }

    let proxy = event_loop.create_proxy();
    let mut tray_ctrl = TrayController::new(&settings, proxy.clone());

    // Shared WebContext & Pre-warmed Windows
    let mut web_context = WebContext::new(app_cache_dir());

    let badge_win = build_overlay_window(&event_loop, "QuickTranslateVN_Badge", 38.0, 38.0, true);
    let badge_ipc_proxy = proxy.clone();
    let _badge_webview = WebViewBuilder::new_with_web_context(&mut web_context)
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
    let popup_webview = WebViewBuilder::new_with_web_context(&mut web_context)
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
        let wv = WebViewBuilder::new_with_web_context(&mut web_context)
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
    let shortcut_webview = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_html(SHORTCUT_HTML)
        .with_ipc_handler(move |req| {
            let _ = shortcut_ipc_proxy.send_event(UserEvent::IpcMessage(req.body().to_string()));
        })
        .build(&shortcut_win)
        .expect("failed to build shortcut recorder webview");

    let switches = Arc::new(HookSwitches {
        enable_badge: AtomicBool::new(settings.show_floating_badge),
        enable_double_copy: AtomicBool::new(settings.enable_double_copy),
    });
    start_global_hooks(proxy.clone(), switches.clone());

    let translator = Arc::new(Translator::new());
    let req_counter = Arc::new(AtomicU64::new(0));
    let busy_capturing = Arc::new(AtomicBool::new(false));
    let mut state = AppState {
        source_lang: "en".to_string(),
        target_lang: "vi".to_string(),
        ..Default::default()
    };

    trim_idle_memory();

    event_loop.run(move |event, _, control_flow| {
        let _ = (&_instance_guard, &web_context);
        *control_flow = match state.badge_hide_deadline {
            Some(dl) => ControlFlow::WaitUntil(dl),
            None => ControlFlow::Wait,
        };

        if let Ok(menu_ev) = MenuEvent::receiver().try_recv() {
            if let Some(action) = tray_ctrl.map_menu_click(&menu_ev.id) {
                match action {
                    TrayAction::TriggerOcrNow => {
                        let _ = proxy.send_event(UserEvent::HotkeyOcr);
                    }
                    TrayAction::OpenOcrShortcutRecorder
                    | TrayAction::OpenSelectionShortcutRecorder => {
                        let target = if matches!(action, TrayAction::OpenOcrShortcutRecorder) {
                            RecorderTarget::Ocr
                        } else {
                            RecorderTarget::Selection
                        };
                        state.recorder_target = Some(target);
                        tray_ctrl.suspend_hotkeys();

                        let cur_spec = match target {
                            RecorderTarget::Ocr => &settings.ocr_shortcut,
                            RecorderTarget::Selection => &settings.shortcut,
                        };
                        if let Ok(js_str) = serde_json::to_string(&format_shortcut_label(cur_spec))
                        {
                            let _ = shortcut_webview.evaluate_script(&format!(
                                "window.__setInitialShortcut({});",
                                js_str
                            ));
                        }
                        center_window_on_primary(&shortcut_win);
                        shortcut_win.set_visible(true);
                        shortcut_win.set_focus();
                    }
                    TrayAction::ResetShortcuts => {
                        settings.ocr_shortcut = "Alt+S".to_string();
                        settings.shortcut = "Alt+Q".to_string();
                        settings.save();
                        tray_ctrl.refresh_hotkeys(&settings);
                    }
                    TrayAction::ToggleBadge(enabled) => {
                        settings.show_floating_badge = enabled;
                        switches.enable_badge.store(enabled, Ordering::Relaxed);
                        settings.save();
                    }
                    TrayAction::ToggleDoubleCopy(enabled) => {
                        settings.enable_double_copy = enabled;
                        switches
                            .enable_double_copy
                            .store(enabled, Ordering::Relaxed);
                        settings.save();
                    }
                    TrayAction::ToggleStartup(enabled) => {
                        AppSettings::set_startup_enabled(enabled);
                    }
                    TrayAction::Quit => {
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }
            }
        }

        match event {
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } if window_id == shortcut_win_id => {
                shortcut_win.set_visible(false);
                state.recorder_target = None;
                tray_ctrl.refresh_hotkeys(&settings);
            }

            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                if let Some(dl) = state.badge_hide_deadline {
                    if Instant::now() >= dl {
                        badge_win.set_visible(false);
                        state.badge_hide_deadline = None;
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
                        state.badge_anchor = (x, y);
                        position_window_near(&badge_win, x, y, 8.0, 10.0);
                        badge_win.set_visible(true);
                        state.badge_hide_deadline =
                            Some(Instant::now() + Duration::from_millis(3800));
                    }
                }

                UserEvent::BadgeClicked => {
                    badge_win.set_visible(false);
                    state.badge_hide_deadline = None;
                    spawn_capture_worker(
                        state.badge_anchor,
                        false,
                        settings.clean_pdf_line_breaks,
                        &req_counter,
                        &busy_capturing,
                        &proxy,
                    );
                }

                UserEvent::HotkeyTranslate => {
                    if !shortcut_win.is_visible() {
                        badge_win.set_visible(false);
                        state.badge_hide_deadline = None;
                        spawn_capture_worker(
                            get_cursor_physical_pos(),
                            false,
                            settings.clean_pdf_line_breaks,
                            &req_counter,
                            &busy_capturing,
                            &proxy,
                        );
                    }
                }

                UserEvent::HotkeyOcr => {
                    if shortcut_win.is_visible() || busy_capturing.load(Ordering::Relaxed) {
                        return;
                    }
                    badge_win.set_visible(false);
                    state.badge_hide_deadline = None;
                    if !state.is_pinned && popup_win.is_visible() {
                        popup_win.set_visible(false);
                    }

                    #[cfg(target_os = "windows")]
                    {
                        let cur = get_cursor_physical_pos();
                        let _ = ocr_webview.evaluate_script("window.__resetOverlay();");
                        window_ext::cover_active_monitor(&ocr_win, cur);
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
                                    let _ = prx.send_event(UserEvent::TextCaptured {
                                        req_id: rid,
                                        anchor: get_cursor_physical_pos(),
                                        cleaned_text: cleaned,
                                    });
                                }
                            }
                            Err(msg) if !msg.is_empty() => {
                                let _ = prx.send_event(UserEvent::OcrFailed {
                                    anchor: get_cursor_physical_pos(),
                                    error_msg: msg,
                                });
                            }
                            _ => {}
                        });
                    }
                }

                UserEvent::DoubleCopy { x, y } => {
                    if !shortcut_win.is_visible() {
                        badge_win.set_visible(false);
                        state.badge_hide_deadline = None;
                        spawn_capture_worker(
                            (x, y),
                            true,
                            settings.clean_pdf_line_breaks,
                            &req_counter,
                            &busy_capturing,
                            &proxy,
                        );
                    }
                }

                UserEvent::TextCaptured {
                    req_id,
                    anchor,
                    cleaned_text,
                } => {
                    state.popup_anchor = anchor;
                    let _ = popup_webview.evaluate_script("window.__renderLoading();");
                    popup_win.set_inner_size(LogicalSize::new(POPUP_WIDTH_DIP, 88.0));
                    position_window_near(&popup_win, anchor.0, anchor.1, 8.0, 14.0);
                    popup_win.set_visible(true);
                    spawn_translation_worker(req_id, cleaned_text, None, &translator, &proxy);
                }

                UserEvent::OcrFailed { anchor, error_msg } => {
                    state.popup_anchor = anchor;
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
                    state.badge_hide_deadline = None;
                    if !state.is_pinned && popup_win.is_visible() {
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
                    state.badge_hide_deadline = None;
                    if !state.is_pinned && popup_win.is_visible() {
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
                            state.original_text = parsed.original_text.clone();
                            state.translated_text = parsed.translated_text.clone();
                            state.source_lang = parsed.source_lang.clone();
                            state.target_lang = parsed.target_lang.clone();
                            let _ = popup_webview.evaluate_script(&format!(
                                "window.__renderResult({});",
                                payload_json
                            ));
                        } else if let Ok(quoted) = serde_json::to_string(&payload_json) {
                            let _ = popup_webview
                                .evaluate_script(&format!("window.__renderError({});", quoted));
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
                    let Ok(msg) = serde_json::from_str::<IpcCmd>(&raw) else {
                        return;
                    };
                    match msg.cmd.as_str() {
                        "badge_clicked" => {
                            let _ = proxy.send_event(UserEvent::BadgeClicked);
                        }
                        #[cfg(target_os = "windows")]
                        "ocr_drag_start" => {
                            state.ocr_start_pt = get_cursor_physical_pos();
                        }
                        #[cfg(target_os = "windows")]
                        "ocr_drag_end" => {
                            let end_pt = get_cursor_physical_pos();
                            ocr_win.set_visible(false);

                            let rx = state.ocr_start_pt.0.min(end_pt.0);
                            let ry = state.ocr_start_pt.1.min(end_pt.1);
                            let rw = (end_pt.0 - state.ocr_start_pt.0).abs();
                            let rh = (end_pt.1 - state.ocr_start_pt.1).abs();

                            if rw >= 10 && rh >= 10 {
                                let rid = req_counter.fetch_add(1, Ordering::SeqCst) + 1;
                                let prx = proxy.clone();
                                let clean_pdf = settings.clean_pdf_line_breaks;
                                std::thread::spawn(move || {
                                    std::thread::sleep(Duration::from_millis(40));
                                    match ocr::capture_rect_and_ocr(rx, ry, rw, rh) {
                                        Ok(raw_ocr) => {
                                            let cleaned = cleaner::clean_text(&raw_ocr, clean_pdf);
                                            if !cleaned.is_empty() {
                                                let _ = prx.send_event(UserEvent::TextCaptured {
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
                            if let (Some(target), Some(sc)) =
                                (state.recorder_target.take(), msg.shortcut)
                            {
                                match target {
                                    RecorderTarget::Ocr => settings.ocr_shortcut = sc,
                                    RecorderTarget::Selection => settings.shortcut = sc,
                                }
                                settings.save();
                            }
                            tray_ctrl.refresh_hotkeys(&settings);
                        }
                        "shortcut_cancel" => {
                            shortcut_win.set_visible(false);
                            state.recorder_target = None;
                            tray_ctrl.refresh_hotkeys(&settings);
                        }
                        "drag_window" => {
                            let _ = popup_win.drag_window();
                        }
                        "resize" => {
                            if let Some(h) = msg.height {
                                popup_win.set_inner_size(LogicalSize::new(POPUP_WIDTH_DIP, h));
                                if !state.is_pinned {
                                    position_window_near(
                                        &popup_win,
                                        state.popup_anchor.0,
                                        state.popup_anchor.1,
                                        8.0,
                                        14.0,
                                    );
                                }
                            }
                        }
                        "swap_lang" => {
                            if !state.original_text.is_empty() {
                                let next_target = if state.target_lang == "vi" {
                                    "en"
                                } else {
                                    "vi"
                                };
                                let _ = popup_webview.evaluate_script("window.__renderLoading();");
                                let rid = req_counter.fetch_add(1, Ordering::SeqCst) + 1;
                                spawn_translation_worker(
                                    rid,
                                    state.original_text.clone(),
                                    Some(next_target.to_string()),
                                    &translator,
                                    &proxy,
                                );
                            }
                        }
                        "speak" => {
                            if !state.original_text.is_empty() {
                                let tr = translator.clone();
                                let orig = state.original_text.clone();
                                let lang = state.source_lang.clone();
                                let prx = proxy.clone();
                                std::thread::spawn(move || {
                                    if let Ok(uri) = tr.fetch_tts_data_uri(&orig, &lang) {
                                        let _ = prx
                                            .send_event(UserEvent::TtsAudioReady { data_uri: uri });
                                    }
                                });
                            }
                        }
                        "copy" => {
                            if !state.translated_text.is_empty() {
                                if let Ok(mut cb) = Clipboard::new() {
                                    let _ = cb.set_text(state.translated_text.clone());
                                }
                            }
                        }
                        "replace" => {
                            if !state.translated_text.is_empty() {
                                let to_insert = state.translated_text.clone();
                                popup_win.set_visible(false);
                                std::thread::spawn(move || {
                                    std::thread::sleep(Duration::from_millis(30));
                                    let _ = capture::replace_selected_text(&to_insert);
                                });
                            }
                        }
                        "pin" => {
                            state.is_pinned = msg.pinned.unwrap_or(false);
                        }
                        "close" => {
                            popup_win.set_visible(false);
                            trim_idle_memory();
                        }
                        _ => {}
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
    proxy: &EventLoopProxy<UserEvent>,
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

fn spawn_translation_worker(
    req_id: u64,
    text: String,
    force_target: Option<String>,
    translator: &Arc<Translator>,
    proxy: &EventLoopProxy<UserEvent>,
) {
    let tr = translator.clone();
    let prx = proxy.clone();
    std::thread::spawn(move || {
        let payload = match tr.translate(&text, force_target.as_deref()) {
            Ok(r) => serde_json::to_string(&r).unwrap_or_default(),
            Err(e) => e,
        };
        let _ = prx.send_event(UserEvent::TranslationCompleted {
            req_id,
            payload_json: payload,
        });
    });
}
