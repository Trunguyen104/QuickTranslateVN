//! Cross-platform overlay window builders and multi-monitor DPI-aware placement.

use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Window, WindowBuilder};

#[cfg(target_os = "windows")]
use tao::platform::windows::WindowBuilderExtWindows;

use crate::hooks::{configure_non_activating_window, UserEvent};

pub const POPUP_WIDTH_DIP: f64 = 384.0;

pub fn build_overlay_window(
    event_loop: &EventLoopWindowTarget<UserEvent>,
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

    let win = wb
        .build(event_loop)
        .expect("failed to build overlay window");
    if non_activating {
        configure_non_activating_window(&win);
    }
    win
}

pub fn build_shortcut_dialog_window(event_loop: &EventLoopWindowTarget<UserEvent>) -> Window {
    WindowBuilder::new()
        .with_title("QuickTranslateVN — Đặt phím tắt")
        .with_inner_size(LogicalSize::new(340.0, 178.0))
        .with_resizable(false)
        .with_always_on_top(true)
        .with_visible(false)
        .build(event_loop)
        .expect("failed to build shortcut recorder window")
}

pub fn center_window_on_primary(window: &Window) {
    if let Some(mon) = window
        .current_monitor()
        .or_else(|| window.primary_monitor())
    {
        let m_pos = mon.position();
        let m_size = mon.size();
        let w_size = window.outer_size();
        let x = m_pos.x + ((m_size.width as i32 - w_size.width as i32) / 2).max(20);
        let y = m_pos.y + ((m_size.height as i32 - w_size.height as i32) / 2).max(20);
        window.set_outer_position(PhysicalPosition::new(x, y));
    }
}

#[cfg(target_os = "windows")]
pub fn cover_active_monitor(window: &Window, cursor: (i32, i32)) {
    let target_mon = window
        .available_monitors()
        .find(|mon| {
            let p = mon.position();
            let s = mon.size();
            cursor.0 >= p.x
                && cursor.0 < p.x + s.width as i32
                && cursor.1 >= p.y
                && cursor.1 < p.y + s.height as i32
        })
        .or_else(|| window.current_monitor())
        .or_else(|| window.primary_monitor());

    if let Some(mon) = target_mon {
        let p = mon.position();
        let s = mon.size();
        window.set_outer_position(PhysicalPosition::new(p.x, p.y));
        window.set_inner_size(PhysicalSize::new(s.width, s.height));
    }
}

pub fn position_window_near(
    window: &Window,
    ax: i32,
    ay: i32,
    offset_dip_x: f64,
    offset_dip_y: f64,
) {
    let target_monitor = window
        .available_monitors()
        .find(|mon| {
            let pos = mon.position();
            let size = mon.size();
            ax >= pos.x
                && ax < pos.x + size.width as i32
                && ay >= pos.y
                && ay < pos.y + size.height as i32
        })
        .or_else(|| window.current_monitor())
        .or_else(|| window.primary_monitor());

    let scale = target_monitor
        .as_ref()
        .map_or_else(|| window.scale_factor(), |m| m.scale_factor());

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
