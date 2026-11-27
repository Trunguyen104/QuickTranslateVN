//! System Tray menu & dynamic Global Hotkey registration controller.

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tao::event_loop::EventLoopProxy;
use tray_icon::menu::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::hooks::UserEvent;
use crate::icon::build_tray_icon;
use crate::settings::AppSettings;
use crate::shortcut::{format_shortcut_label, parse_shortcut};

pub enum TrayAction {
    TriggerOcrNow,
    OpenOcrShortcutRecorder,
    OpenSelectionShortcutRecorder,
    ResetShortcuts,
    ToggleBadge(bool),
    ToggleDoubleCopy(bool),
    ToggleStartup(bool),
    Quit,
}

pub struct TrayController {
    _tray_icon: Option<TrayIcon>,
    hotkey_manager: Option<GlobalHotKeyManager>,
    active_sel_hk: Option<HotKey>,
    active_ocr_hk: Option<HotKey>,
    active_sel_id: Arc<AtomicU32>,
    active_ocr_id: Arc<AtomicU32>,

    item_ocr_now: MenuItem,
    item_change_ocr_sc: MenuItem,
    item_change_sel_sc: MenuItem,
    pub item_badge: CheckMenuItem,
    pub item_dbl_copy: CheckMenuItem,
    pub item_startup: CheckMenuItem,

    id_ocr_now: MenuId,
    id_change_ocr_sc: MenuId,
    id_change_sel_sc: MenuId,
    id_reset_sc: MenuId,
    id_badge: MenuId,
    id_dbl_copy: MenuId,
    id_startup: MenuId,
    id_quit: MenuId,
}

impl TrayController {
    pub fn new(settings: &AppSettings, proxy: EventLoopProxy<UserEvent>) -> Self {
        let hotkey_manager = GlobalHotKeyManager::new().ok();
        let active_sel_id = Arc::new(AtomicU32::new(0));
        let active_ocr_id = Arc::new(AtomicU32::new(0));

        let active_sel_hk = parse_shortcut(&settings.shortcut);
        let active_ocr_hk = parse_shortcut(&settings.ocr_shortcut);

        if let Some(mgr) = &hotkey_manager {
            if let Some(hk) = active_sel_hk {
                if mgr.register(hk).is_ok() {
                    active_sel_id.store(hk.id(), Ordering::SeqCst);
                }
            }
            if let Some(hk) = active_ocr_hk {
                if mgr.register(hk).is_ok() {
                    active_ocr_id.store(hk.id(), Ordering::SeqCst);
                }
            }
        }

        let sel_ref = active_sel_id.clone();
        let ocr_ref = active_ocr_id.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
            if ev.state == HotKeyState::Pressed {
                let sid = sel_ref.load(Ordering::SeqCst);
                let oid = ocr_ref.load(Ordering::SeqCst);
                if sid != 0 && ev.id == sid {
                    let _ = proxy.send_event(UserEvent::HotkeyTranslate);
                } else if oid != 0 && ev.id == oid {
                    let _ = proxy.send_event(UserEvent::HotkeyOcr);
                }
            }
        }));

        let ocr_lbl = format_shortcut_label(&settings.ocr_shortcut);
        let sel_lbl = format_shortcut_label(&settings.shortcut);

        let tray_menu = Menu::new();
        let item_ocr_now = MenuItem::new(
            format!("Chụp vùng màn hình dịch OCR ({})", ocr_lbl),
            true,
            None,
        );

        let shortcut_submenu = Submenu::new("Đổi phím tắt nhanh", true);
        let item_change_ocr_sc = MenuItem::new(
            format!("Đặt phím tắt Chụp màn hình OCR (Hiện tại: {})...", ocr_lbl),
            true,
            None,
        );
        let item_change_sel_sc = MenuItem::new(
            format!("Đặt phím tắt Dịch chữ bôi đen (Hiện tại: {})...", sel_lbl),
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
                ocr_lbl, sel_lbl
            ))
            .with_icon(build_tray_icon())
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

        Self {
            _tray_icon: tray_icon,
            hotkey_manager,
            active_sel_hk,
            active_ocr_hk,
            active_sel_id,
            active_ocr_id,
            item_ocr_now,
            item_change_ocr_sc,
            item_change_sel_sc,
            item_badge,
            item_dbl_copy,
            item_startup,
            id_ocr_now,
            id_change_ocr_sc,
            id_change_sel_sc,
            id_reset_sc,
            id_badge,
            id_dbl_copy,
            id_startup,
            id_quit,
        }
    }

    pub fn map_menu_click(&self, id: &MenuId) -> Option<TrayAction> {
        if id == &self.id_ocr_now {
            Some(TrayAction::TriggerOcrNow)
        } else if id == &self.id_change_ocr_sc {
            Some(TrayAction::OpenOcrShortcutRecorder)
        } else if id == &self.id_change_sel_sc {
            Some(TrayAction::OpenSelectionShortcutRecorder)
        } else if id == &self.id_reset_sc {
            Some(TrayAction::ResetShortcuts)
        } else if id == &self.id_badge {
            Some(TrayAction::ToggleBadge(self.item_badge.is_checked()))
        } else if id == &self.id_dbl_copy {
            Some(TrayAction::ToggleDoubleCopy(
                self.item_dbl_copy.is_checked(),
            ))
        } else if id == &self.id_startup {
            Some(TrayAction::ToggleStartup(self.item_startup.is_checked()))
        } else if id == &self.id_quit {
            Some(TrayAction::Quit)
        } else {
            None
        }
    }

    pub fn suspend_hotkeys(&self) {
        self.active_sel_id.store(0, Ordering::SeqCst);
        self.active_ocr_id.store(0, Ordering::SeqCst);
        if let Some(mgr) = &self.hotkey_manager {
            if let Some(hk) = self.active_sel_hk {
                let _ = mgr.unregister(hk);
            }
            if let Some(hk) = self.active_ocr_hk {
                let _ = mgr.unregister(hk);
            }
        }
    }

    pub fn refresh_hotkeys(&mut self, settings: &AppSettings) {
        self.suspend_hotkeys();

        self.active_sel_hk = parse_shortcut(&settings.shortcut);
        self.active_ocr_hk = parse_shortcut(&settings.ocr_shortcut);

        if let Some(mgr) = &self.hotkey_manager {
            if let Some(hk) = self.active_sel_hk {
                if mgr.register(hk).is_ok() {
                    self.active_sel_id.store(hk.id(), Ordering::SeqCst);
                }
            }
            if let Some(hk) = self.active_ocr_hk {
                if mgr.register(hk).is_ok() {
                    self.active_ocr_id.store(hk.id(), Ordering::SeqCst);
                }
            }
        }

        let ocr_lbl = format_shortcut_label(&settings.ocr_shortcut);
        let sel_lbl = format_shortcut_label(&settings.shortcut);

        self.item_ocr_now
            .set_text(format!("Chụp vùng màn hình dịch OCR ({})", ocr_lbl));
        self.item_change_ocr_sc.set_text(format!(
            "Đặt phím tắt Chụp màn hình OCR (Hiện tại: {})...",
            ocr_lbl
        ));
        self.item_change_sel_sc.set_text(format!(
            "Đặt phím tắt Dịch chữ bôi đen (Hiện tại: {})...",
            sel_lbl
        ));

        if let Some(tray) = &self._tray_icon {
            let _ = tray.set_tooltip(Some(format!(
                "QuickTranslateVN — OCR: {} | Dịch chọn: {}",
                ocr_lbl, sel_lbl
            )));
        }
    }
}
