use crate::state::{AppState, DisplayPayload};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Rect, WebviewUrl, WebviewWindowBuilder,
};
use tauri::window::Color;

static MENU_HIDDEN_AT: AtomicU64 = AtomicU64::new(0);
static MENU_SHOWN_AT: AtomicU64 = AtomicU64::new(0);
static MENU_FOCUS_RETRY: AtomicBool = AtomicBool::new(false);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as u64).unwrap_or(0)
}

pub fn open_view(
    app: &AppHandle,
    label: &str,
    view: &str,
    title: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    decorations: bool,
    transparent: bool,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_position(LogicalPosition::new(x, y));
        let _ = window.set_size(LogicalSize::new(width, height));
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = app.emit("refresh", label);
        return Ok(());
    }
    let capture = label.starts_with("overlay-");
    let url = WebviewUrl::App(format!("index.html?view={view}").into());
    let builder = WebviewWindowBuilder::new(app, label, url)
        .title(title)
        .inner_size(width, height)
        .position(x, y)
        .decorations(decorations)
        .transparent(transparent)
        .background_color(if transparent { Color(0, 0, 0, 0) } else { Color(28, 28, 30, 255) })
        .always_on_top(transparent || label.starts_with("pin-") || capture || label == "hud" || label == "donut")
        .skip_taskbar(transparent || label == "hud" || label == "donut" || label.starts_with("pin-"))
        .resizable(decorations)
        .shadow(!transparent && !capture)
        .visible(true)
        .focused(label != "hud")
        .accept_first_mouse(capture || label == "donut")
        .content_protected(capture);
    builder.build().map_err(|err| err.to_string())?;
    Ok(())
}

pub fn dismiss_donut(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut donut) = state.donut.lock() {
            *donut = None;
        }
    }
    if let Some(window) = app.get_webview_window("donut") {
        let _ = window.hide();
        let _ = window.destroy();
    }
}

pub fn hide_labeled(app: &AppHandle, prefix: &str) {
    let mut hid_overlay = false;
    for (label, window) in app.webview_windows() {
        if label.starts_with(prefix) || label == prefix {
            let _ = window.hide();
            if label.starts_with("overlay") {
                hid_overlay = true;
            }
        }
    }
    if hid_overlay {
        crate::set_tray_capturing(app, false);
    }
}

pub fn show_overlays(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let displays = state.displays.lock().map_err(|err| err.to_string())?.clone();
    drop(state);
    for display in displays {
        show_overlay(app, &display)?;
    }
    crate::set_tray_capturing(app, true);
    Ok(())
}

fn show_overlay(app: &AppHandle, display: &DisplayPayload) -> Result<(), String> {
    let scale = display.scale.max(1.0) as f64;
    // Display origin is already in points. Width and height on the payload are pixels.
    let width = (display.width as f64 / scale).round().max(1.0);
    let height = (display.height as f64 / scale).round().max(1.0);
    let x = display.x as f64;
    let y = display.y as f64;
    let label = format!("overlay-{}", display.id);
    open_view(
        app,
        &label,
        &format!("overlay&display={}", display.id),
        "Capture",
        x,
        y,
        width,
        height,
        false,
        false,
    )?;
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.set_content_protected(true);
        seat_overlay(&window, x, y, width, height);
    }
    Ok(())
}

fn seat_overlay(window: &tauri::WebviewWindow, x: f64, y: f64, width: f64, height: f64) {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSScreen, NSWindow, NSWindowCollectionBehavior, NSWindowSharingType};
        use objc2_core_foundation::{CGPoint, CGRect, CGSize};
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Ok(raw) = window.ns_window() else {
            return;
        };
        let Some(ptr) = std::ptr::NonNull::new(raw as *mut NSWindow) else {
            return;
        };
        let ns = unsafe { ptr.as_ref() };
        let primary_h = NSScreen::mainScreen(mtm)
            .map(|screen| screen.frame().size.height)
            .unwrap_or(height);
        let frame = CGRect::new(
            CGPoint::new(x, primary_h - (y + height)),
            CGSize::new(width, height),
        );
        ns.setFrame_display(frame, true);
        ns.setLevel(1000);
        ns.setSharingType(NSWindowSharingType::None);
        ns.setHidesOnDeactivate(false);
        ns.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        ns.orderFrontRegardless();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, x, y, width, height);
    }
}

pub fn place_near_cursor(app: &AppHandle, label: &str, view: &str, size: f64) -> Result<(), String> {
    let (cx, cy) = shotsmith_platform::cursor_position();
    let monitors = app.available_monitors().unwrap_or_default();
    let monitor = monitors.iter().find(|item| {
        let scale = item.scale_factor().max(1.0);
        let origin = item.position().to_logical::<f64>(scale);
        let bounds = item.size().to_logical::<f64>(scale);
        cx as f64 >= origin.x
            && cy as f64 >= origin.y
            && (cx as f64) < origin.x + bounds.width
            && (cy as f64) < origin.y + bounds.height
    });
    let monitor = monitor.or(monitors.first());
    let margin = 8.0;
    let (min_x, min_y, max_x, max_y) = if let Some(monitor) = monitor {
        let scale = monitor.scale_factor().max(1.0);
        let origin = monitor.position().to_logical::<f64>(scale);
        let bounds = monitor.size().to_logical::<f64>(scale);
        (
            origin.x + margin,
            origin.y + margin,
            origin.x + bounds.width - size - margin,
            origin.y + bounds.height - size - margin,
        )
    } else {
        (margin, margin, 1280.0 - size, 800.0 - size)
    };
    let x = (cx as f64 - size / 2.0).clamp(min_x, max_x.max(min_x));
    let y = (cy as f64 - size / 2.0).clamp(min_y, max_y.max(min_y));
    open_view(app, label, view, "Shotsmith", x, y, size, size, false, true)?;
    if label == "donut" {
        if let Some(window) = app.get_webview_window(label) {
            clear_round_window(&window);
        }
    }
    Ok(())
}

fn clear_round_window(window: &tauri::WebviewWindow) {
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSWindow;
        if MainThreadMarker::new().is_none() {
            return;
        }
        let Ok(raw) = window.ns_window() else {
            return;
        };
        let Some(ptr) = std::ptr::NonNull::new(raw as *mut NSWindow) else {
            return;
        };
        let ns = unsafe { ptr.as_ref() };
        ns.setOpaque(false);
        ns.setHasShadow(false);
    }
}

pub fn open_history(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("history") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = app.emit("refresh", "history");
        return Ok(());
    }
    let url = WebviewUrl::App("index.html?view=history".into());
    let builder = WebviewWindowBuilder::new(app, "history", url)
        .title("History")
        .inner_size(960.0, 640.0)
        .min_inner_size(720.0, 480.0)
        .position(140.0, 80.0)
        .decorations(true)
        .resizable(true)
        .background_color(Color(255, 255, 255, 255))
        .visible(true)
        .focused(true);
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    builder.build().map_err(|err| err.to_string())?;
    Ok(())
}

pub fn open_settings(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return Ok(());
    }
    let url = WebviewUrl::App("index.html?view=settings".into());
    let builder = WebviewWindowBuilder::new(app, "settings", url)
        .title("Settings")
        .inner_size(1040.0, 720.0)
        .min_inner_size(880.0, 640.0)
        .position(160.0, 80.0)
        .decorations(true)
        .resizable(true)
        .background_color(Color(255, 255, 255, 255))
        .visible(true)
        .focused(true);
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    builder.build().map_err(|err| err.to_string())?;
    Ok(())
}

pub fn open_editor(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("editor") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = app.emit("refresh", "editor");
        return Ok(());
    }
    let url = WebviewUrl::App("index.html?view=editor".into());
    let builder = WebviewWindowBuilder::new(app, "editor", url)
        .title("Shotsmith")
        .inner_size(1360.0, 828.0)
        .min_inner_size(960.0, 640.0)
        .position(80.0, 48.0)
        .decorations(true)
        .resizable(true)
        .background_color(Color(255, 255, 255, 255))
        .visible(true)
        .focused(true);
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    builder.build().map_err(|err| err.to_string())?;
    Ok(())
}

pub fn toggle_status_menu(app: &AppHandle, rect: Rect) {
    if let Some(window) = app.get_webview_window("status-menu") {
        if window.is_visible().unwrap_or(false) {
            MENU_HIDDEN_AT.store(now_ms(), Ordering::Relaxed);
            let _ = window.hide();
            return;
        }
    }
    if now_ms().saturating_sub(MENU_HIDDEN_AT.load(Ordering::Relaxed)) < 280 {
        return;
    }
    // The status item click is not an activating click. Showing the panel
    // inside that mouseDown leaves it visible but not key: hover never
    // tracks and the translucent background stays washed out. Open it on
    // the next turn, after the click has finished.
    let app = app.clone();
    let queued = app.clone();
    let _ = app.run_on_main_thread(move || {
        if place_status_menu(&queued, rect).is_err() {
            return;
        }
        MENU_SHOWN_AT.store(now_ms(), Ordering::Relaxed);
        MENU_FOCUS_RETRY.store(true, Ordering::Relaxed);
        if let Some(window) = queued.get_webview_window("status-menu") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    });
}

fn place_status_menu(app: &AppHandle, rect: Rect) -> Result<(), String> {
    const WIN_W: f64 = 396.0;
    const WIN_H: f64 = 560.0;
    const PAD: f64 = 28.0;
    let monitors = app.available_monitors().unwrap_or_default();
    let anchor = rect.position.to_physical::<f64>(1.0);
    let monitor = monitors.iter().find(|item| {
        let origin = item.position();
        let size = item.size();
        anchor.x >= origin.x as f64
            && anchor.y >= origin.y as f64
            && anchor.x < origin.x as f64 + size.width as f64
            && anchor.y < origin.y as f64 + size.height as f64
    });
    let monitor = monitor.or(monitors.first());
    let scale = monitor.map(|item| item.scale_factor()).unwrap_or(1.0).max(1.0);
    let origin = rect.position.to_logical::<f64>(scale);
    let icon = rect.size.to_logical::<f64>(scale);
    let mut x = origin.x + icon.width / 2.0 - WIN_W / 2.0;
    let mut y = origin.y + icon.height + 5.0 - PAD;
    if let Some(monitor) = monitor {
        let origin_x = monitor.position().x as f64 / scale;
        let width = monitor.size().width as f64 / scale;
        let left = origin_x + 8.0;
        let top = monitor.position().y as f64 / scale;
        let right = origin_x + width - WIN_W - 8.0;
        x = x.clamp(left, right.max(left));
        y = y.max(top);
    }
    if let Some(window) = app.get_webview_window("status-menu") {
        let _ = window.set_position(LogicalPosition::new(x, y));
        let _ = app.emit("refresh", "status-menu");
        return Ok(());
    }
    let url = WebviewUrl::App("index.html?view=menu".into());
    let window = WebviewWindowBuilder::new(app, "status-menu", url)
        .title("Shotsmith")
        .inner_size(WIN_W, WIN_H)
        .position(x, y)
        .decorations(false)
        .transparent(true)
        .background_color(Color(0, 0, 0, 0))
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .focused(false)
        .accept_first_mouse(true)
        .build()
        .map_err(|err| err.to_string())?;
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(false) = event {
            let Some(menu) = handle.get_webview_window("status-menu") else {
                return;
            };
            if !menu.is_visible().unwrap_or(false) {
                return;
            }
            // The status-item click can resign key once, just after we show
            // the panel. Take focus back instead of dismissing it.
            let since = now_ms().saturating_sub(MENU_SHOWN_AT.load(Ordering::Relaxed));
            if since < 250 && MENU_FOCUS_RETRY.swap(false, Ordering::Relaxed) {
                let _ = menu.set_focus();
                return;
            }
            MENU_HIDDEN_AT.store(now_ms(), Ordering::Relaxed);
            let _ = menu.hide();
        }
    });
    Ok(())
}

static HUD_TOKEN: AtomicU64 = AtomicU64::new(0);

pub fn place_hud(app: &AppHandle) -> Result<(), String> {
    const W: f64 = 360.0;
    const H: f64 = 84.0;
    let (cursor_x, cursor_y) = shotsmith_platform::cursor_position();
    let monitors = app.available_monitors().unwrap_or_default();
    let monitor = monitors.iter().find(|item| {
        let scale = item.scale_factor();
        let origin = item.position().to_logical::<f64>(scale);
        let size = item.size().to_logical::<f64>(scale);
        cursor_x as f64 >= origin.x
            && cursor_y as f64 >= origin.y
            && (cursor_x as f64) < origin.x + size.width
            && (cursor_y as f64) < origin.y + size.height
    });
    let monitor = monitor.or(monitors.first());
    let (x, y) = if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let origin = monitor.position().to_logical::<f64>(scale);
        let size = monitor.size().to_logical::<f64>(scale);
        (origin.x + size.width - W - 24.0, origin.y + size.height - H - 72.0)
    } else {
        (80.0, 80.0)
    };
    open_view(app, "hud", "hud", "Shotsmith", x, y, W, H, false, true)
}

pub fn arm_hud_dismiss(app: &AppHandle) {
    let token = HUD_TOKEN.fetch_add(1, Ordering::Relaxed) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2600));
        if HUD_TOKEN.load(Ordering::Relaxed) != token {
            return;
        }
        if let Some(window) = app.get_webview_window("hud") {
            let _ = window.hide();
        }
    });
}
