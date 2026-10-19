use crate::state::{AppState, DisplayPayload};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};

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
    let url = WebviewUrl::App(format!("index.html?view={view}").into());
    WebviewWindowBuilder::new(app, label, url)
        .title(title)
        .inner_size(width, height)
        .position(x, y)
        .decorations(decorations)
        .transparent(transparent)
        .always_on_top(transparent || label.starts_with("pin-") || label.starts_with("overlay-") || label == "hud" || label == "donut")
        .skip_taskbar(transparent || label == "hud" || label == "donut" || label.starts_with("pin-"))
        .resizable(decorations)
        .shadow(!transparent)
        .visible(true)
        .focused(label != "hud")
        .build()
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn hide_labeled(app: &AppHandle, prefix: &str) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(prefix) || label == prefix {
            let _ = window.hide();
        }
    }
}

pub fn show_overlays(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let displays = state.displays.lock().map_err(|err| err.to_string())?.clone();
    drop(state);
    for display in displays {
        show_overlay(app, &display)?;
    }
    Ok(())
}

fn show_overlay(app: &AppHandle, display: &DisplayPayload) -> Result<(), String> {
    let scale = display.scale.max(1.0) as f64;
    let width = display.width as f64 / scale;
    let height = display.height as f64 / scale;
    let x = display.x as f64 / scale;
    let y = display.y as f64 / scale;
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
    )
}

pub fn place_near_cursor(app: &AppHandle, label: &str, view: &str, size: f64) -> Result<(), String> {
    let (cx, cy) = shotsmith_platform::cursor_position();
    let state = app.state::<AppState>();
    let displays = state.displays.lock().map_err(|err| err.to_string())?.clone();
    drop(state);
    let display = displays.iter().find(|display| {
        cx >= display.x
            && cy >= display.y
            && cx < display.x + display.width as i32
            && cy < display.y + display.height as i32
    });
    let (scale, origin_x, origin_y, limit_w, limit_h) = display
        .map(|display| {
            (
                display.scale.max(1.0) as f64,
                display.x as f64,
                display.y as f64,
                display.width as f64,
                display.height as f64,
            )
        })
        .unwrap_or((1.0, 0.0, 0.0, 1280.0, 800.0));
    let mut x = cx as f64 / scale - size / 2.0;
    let mut y = cy as f64 / scale - size / 2.0;
    let min_x = origin_x / scale;
    let min_y = origin_y / scale;
    let max_x = min_x + limit_w / scale - size;
    let max_y = min_y + limit_h / scale - size;
    x = x.clamp(min_x, max_x.max(min_x));
    y = y.clamp(min_y, max_y.max(min_y));
    open_view(app, label, view, "Shotsmith", x, y, size, size, false, true)
}

pub fn place_hud(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let displays = state.displays.lock().map_err(|err| err.to_string())?.clone();
    drop(state);
    let display = displays.iter().find(|item| item.x == 0 && item.y == 0).or(displays.first());
    let (x, y) = if let Some(display) = display {
        let scale = display.scale.max(1.0) as f64;
        let width = display.width as f64 / scale;
        let height = display.height as f64 / scale;
        (
            display.x as f64 / scale + width - 340.0,
            display.y as f64 / scale + height - 110.0,
        )
    } else {
        (40.0, 40.0)
    };
    open_view(app, "hud", "hud", "Shotsmith", x, y, 320.0, 84.0, false, true)
}
