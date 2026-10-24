use crate::state::{AppState, DisplayPayload};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::window::Color;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Rect, WebviewUrl,
    WebviewWindowBuilder,
};

pub fn is_document_window(label: &str) -> bool {
    matches!(label, "onboarding" | "settings" | "history") || label.starts_with("editor-")
}

pub fn reveal_documents(app: &AppHandle) {
    let mut focus = None;
    for (label, window) in app.webview_windows() {
        if !is_document_window(&label) {
            continue;
        }
        let _ = window.unminimize();
        let _ = window.show();
        if label.starts_with("editor-") || focus.is_none() {
            focus = Some(window);
        }
    }
    if let Some(window) = focus {
        let _ = window.set_focus();
    }
}

static MENU_HIDDEN_AT: AtomicU64 = AtomicU64::new(0);
static MENU_SHOWN_AT: AtomicU64 = AtomicU64::new(0);
static MENU_FOCUS_RETRY: AtomicBool = AtomicBool::new(false);
static OVERLAY_UP: AtomicBool = AtomicBool::new(false);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
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
    let capture = label.starts_with("overlay-");
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_position(LogicalPosition::new(x, y));
        let _ = window.set_size(LogicalSize::new(width, height));
        // The capture window is shown only after it is marked as unable to
        // enter a fullscreen Space. Showing it here does that first.
        if !capture {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        let _ = app.emit("refresh", label);
        if is_document_window(label) {
            apply_appearance(app);
            sync_dock(app);
        }
        return Ok(());
    }
    let chrome = is_document_window(label);
    let url = WebviewUrl::App(format!("index.html?view={view}").into());
    let builder = WebviewWindowBuilder::new(app, label, url)
        .title(title)
        .inner_size(width, height)
        .position(x, y)
        .decorations(decorations)
        .transparent(transparent)
        .background_color(if transparent {
            Color(0, 0, 0, 0)
        } else {
            Color(28, 28, 30, 255)
        })
        .always_on_top(
            transparent
                || label.starts_with("pin-")
                || capture
                || label == "hud"
                || label == "donut",
        )
        .skip_taskbar(
            transparent
                || capture
                || label == "hud"
                || label == "donut"
                || label.starts_with("pin-"),
        )
        .resizable(decorations)
        .shadow(!transparent && !capture)
        .visible(!chrome && !capture)
        .focused(label != "hud")
        .accept_first_mouse(capture || label == "donut" || label == "hud")
        .content_protected(capture);
    let window = builder.build().map_err(|err| err.to_string())?;
    if transparent || label == "donut" || label == "hud" {
        clear_round_window(&window);
    }
    if chrome {
        watch_window(app, &window);
        list_in_dock(&window);
        apply_appearance(app);
        let _ = window.show();
        let _ = window.set_focus();
        sync_dock(app);
    }
    Ok(())
}

fn stored_appearance(app: &AppHandle) -> String {
    app.try_state::<AppState>()
        .and_then(|state| {
            state
                .settings
                .lock()
                .ok()
                .map(|settings| settings.appearance.clone())
        })
        .unwrap_or_else(|| "system".into())
}

/// Header rows are 52pt. The system keeps the traffic lights in the short
/// titlebar, so a two-line title ends up with the subtitle hanging below them.
fn place_traffic_lights(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSAutoresizingMaskOptions, NSWindow, NSWindowButton};
        use objc2_core_foundation::CGPoint;
        const BAR: f64 = 52.0;
        let Some(_main) = MainThreadMarker::new() else {
            return;
        };
        let Ok(raw) = window.ns_window() else {
            return;
        };
        let Some(ptr) = std::ptr::NonNull::new(raw as *mut NSWindow) else {
            return;
        };
        let ns = unsafe { ptr.as_ref() };
        let Some(content) = ns.contentView() else {
            return;
        };
        let content_height = content.frame().size.height;
        for kind in [
            NSWindowButton::CloseButton,
            NSWindowButton::MiniaturizeButton,
            NSWindowButton::ZoomButton,
        ] {
            let Some(button) = ns.standardWindowButton(kind) else {
                continue;
            };
            let frame = button.frame();
            let Some(parent) = (unsafe { button.superview() }) else {
                continue;
            };
            let in_window = parent.convertPoint_toView(frame.origin, None);
            let y = content_height - BAR + (BAR - frame.size.height) / 2.0;
            let placed = parent.convertPoint_fromView(CGPoint::new(in_window.x, y), None);
            button.setFrameOrigin(placed);
            button.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewMaxXMargin
                    | NSAutoresizingMaskOptions::ViewMinYMargin,
            );
        }
    }
}

fn seat_traffic_lights(window: &tauri::WebviewWindow) {
    place_traffic_lights(window);
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(40));
        let again = window.clone();
        let _ = window.run_on_main_thread(move || place_traffic_lights(&again));
    });
}

fn list_in_dock(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWindow;
        let Ok(raw) = window.ns_window() else {
            return;
        };
        let Some(ptr) = std::ptr::NonNull::new(raw as *mut NSWindow) else {
            return;
        };
        unsafe { ptr.as_ref().setExcludedFromWindowsMenu(false) };
    }
}

fn paint_chrome(window: &tauri::WebviewWindow, appearance: &str) {
    let dark = match appearance {
        "dark" => true,
        "light" => false,
        _ => matches!(window.theme().ok(), Some(tauri::Theme::Dark)),
    };
    let color = if dark {
        Color(28, 28, 30, 255)
    } else {
        Color(255, 255, 255, 255)
    };
    let _ = window.set_background_color(Some(color));
}

/// Follow the Appearance setting. `system` leaves the window on the OS theme.
pub fn apply_appearance(app: &AppHandle) {
    let appearance = stored_appearance(app);
    let theme = match appearance.as_str() {
        "dark" => Some(tauri::Theme::Dark),
        "light" => Some(tauri::Theme::Light),
        _ => None,
    };
    for (_, window) in app.webview_windows() {
        let _ = window.set_theme(theme);
    }
    for (label, window) in app.webview_windows() {
        if is_document_window(&label) {
            paint_chrome(&window, &appearance);
        }
    }
}

pub fn sync_dock(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let app = app.clone();
        let _ = app.clone().run_on_main_thread(move || apply_dock(&app));
    }
}

fn watch_window(app: &AppHandle, window: &tauri::WebviewWindow) {
    let app = app.clone();
    let label = window.label().to_string();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Destroyed => {
            if let Some(id) = label.strip_prefix("editor-") {
                let id = id.to_string();
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(mut editors) = state.editors.lock() {
                        editors.remove(&id);
                    }
                }
            }
            #[cfg(target_os = "macos")]
            {
                let queued = app.clone();
                let _ = app.run_on_main_thread(move || apply_dock(&queued));
            }
        }
        tauri::WindowEvent::Resized(_)
            if label == "settings" || label == "history" || label.starts_with("editor-") =>
        {
            let queued = app.clone();
            let label = label.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(window) = queued.get_webview_window(&label) {
                    place_traffic_lights(&window);
                }
            });
        }
        tauri::WindowEvent::ThemeChanged(_) => {
            let queued = app.clone();
            let label = label.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(window) = queued.get_webview_window(&label) {
                    paint_chrome(&window, &stored_appearance(&queued));
                }
            });
        }
        _ => {}
    });
}

#[cfg(target_os = "macos")]
fn apply_dock(app: &AppHandle) {
    let pinned = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .settings
                .lock()
                .ok()
                .map(|settings| settings.show_dock_icon)
        })
        .unwrap_or(false);
    let open = app
        .webview_windows()
        .into_iter()
        .any(|(label, window)| is_document_window(&label) && window.is_visible().unwrap_or(false));
    // Accessory apps are omitted from Force Quit. A capture has to stay listed
    // so a stuck overlay can be quit without restarting the Mac.
    let policy = if pinned || open || OVERLAY_UP.load(Ordering::SeqCst) {
        tauri::ActivationPolicy::Regular
    } else {
        tauri::ActivationPolicy::Accessory
    };
    let _ = app.set_activation_policy(policy);
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

pub fn cancel_overlays(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut preset) = state.capture_preset.lock() {
            *preset = None;
        }
    }
    hide_labeled(app, "overlay-");
}

pub fn hide_labeled(app: &AppHandle, prefix: &str) {
    let mut hid_overlay = false;
    for (label, window) in app.webview_windows() {
        if label.starts_with(prefix) || label == prefix {
            if label.starts_with("overlay") {
                // Hiding leaves the screen-sized window in front. Close it.
                release_overlay(&window);
                hid_overlay = true;
                let _ = window.hide();
                let _ = window.destroy();
                continue;
            }
            let _ = window.hide();
        }
    }
    if hid_overlay {
        OVERLAY_UP.store(false, Ordering::SeqCst);
        disarm_overlay_cancel();
        crate::set_tray_capturing(app, false);
        sync_dock(app);
    }
}

pub fn show_overlays(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let displays = state
        .displays
        .lock()
        .map_err(|err| err.to_string())?
        .clone();
    drop(state);
    if displays.is_empty() {
        return Ok(());
    }
    // Accessory apps are missing from Force Quit. Set this before the shield
    // exists, so a stuck capture can still be quit.
    OVERLAY_UP.store(true, Ordering::SeqCst);
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    }
    for display in displays {
        if let Err(err) = show_overlay(app, &display) {
            hide_labeled(app, "overlay-");
            OVERLAY_UP.store(false, Ordering::SeqCst);
            sync_dock(app);
            return Err(err);
        }
    }
    arm_overlay_cancel(app);
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
        let _ = window.show();
    }
    Ok(())
}

fn seat_overlay(window: &tauri::WebviewWindow, x: f64, y: f64, width: f64, height: f64) {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{
            NSPopUpMenuWindowLevel, NSWindow, NSWindowCollectionBehavior, NSWindowSharingType,
        };
        use objc2_core_foundation::{CGPoint, CGRect, CGSize};
        let Some(_main) = MainThreadMarker::new() else {
            return;
        };
        let Ok(raw) = window.ns_window() else {
            return;
        };
        let Some(ptr) = std::ptr::NonNull::new(raw as *mut NSWindow) else {
            return;
        };
        let ns = unsafe { ptr.as_ref() };
        // Flip from top-left display points using the menu-bar display, not
        // whichever screen currently holds the key window.
        let primary_h = primary_display_height().unwrap_or(height);
        let frame = CGRect::new(
            CGPoint::new(x, primary_h - (y + height)),
            CGSize::new(width, height),
        );
        // FullScreenAuxiliary lets a screen-sized window become its own Space.
        // Closing that window leaves the Space up, so other apps open invisibly
        // under it. FullScreenNone keeps the shield on the current Space.
        ns.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenNone
                | NSWindowCollectionBehavior::FullScreenDisallowsTiling
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        ns.setFrame_display(frame, true);
        // Popup level covers the menu bar and stays under Force Quit.
        ns.setLevel(NSPopUpMenuWindowLevel);
        ns.setSharingType(NSWindowSharingType::None);
        ns.setHidesOnDeactivate(false);
        ns.makeKeyAndOrderFront(None);
    }
    #[cfg(target_os = "windows")]
    {
        let _ = (x, y, width, height);
        let _ = window.set_always_on_top(true);
        let _ = window.set_skip_taskbar(true);
        seat_tool_window(window);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (window, x, y, width, height);
    }
}

fn release_overlay(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSNormalWindowLevel, NSWindow, NSWindowStyleMask};
        let Some(_main) = MainThreadMarker::new() else {
            let _ = window.hide();
            return;
        };
        let Ok(raw) = window.ns_window() else {
            let _ = window.hide();
            return;
        };
        let Some(ptr) = std::ptr::NonNull::new(raw as *mut NSWindow) else {
            return;
        };
        let ns = unsafe { ptr.as_ref() };
        let fullscreen = ns.styleMask().contains(NSWindowStyleMask::FullScreen);
        if fullscreen {
            ns.toggleFullScreen(None);
        }
        ns.setLevel(NSNormalWindowLevel);
        ns.setIgnoresMouseEvents(true);
        if !fullscreen {
            ns.orderOut(None);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.hide();
    }
}

#[cfg(target_os = "macos")]
fn primary_display_height() -> Option<f64> {
    use objc2_core_foundation::CGRect;
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> u32;
        fn CGDisplayBounds(display: u32) -> CGRect;
    }
    let height = unsafe { CGDisplayBounds(CGMainDisplayID()).size.height };
    (height > 1.0).then_some(height)
}

fn arm_overlay_cancel(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        arm_overlay_cancel_mac(app);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
}

fn disarm_overlay_cancel() {
    #[cfg(target_os = "macos")]
    {
        disarm_overlay_cancel_mac();
    }
}

#[cfg(target_os = "macos")]
thread_local! {
    static OVERLAY_MONITOR: std::cell::RefCell<Option<objc2::rc::Retained<objc2::runtime::AnyObject>>> = std::cell::RefCell::new(None);
}

#[cfg(target_os = "macos")]
fn arm_overlay_cancel_mac(app: &AppHandle) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags};

    disarm_overlay_cancel_mac();
    let app = app.clone();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        let event_ref = unsafe { event.as_ref() };
        let code = event_ref.keyCode();
        let command_period = code == 47
            && event_ref
                .modifierFlags()
                .contains(NSEventModifierFlags::Command);
        if code == 53 || command_period {
            let queued = app.clone();
            let _ = app.run_on_main_thread(move || cancel_overlays(&queued));
            std::ptr::null_mut()
        } else {
            event.as_ptr()
        }
    });
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)
    };
    if let Some(monitor) = monitor {
        OVERLAY_MONITOR.with(|slot| *slot.borrow_mut() = Some(monitor));
    }
}

#[cfg(target_os = "macos")]
fn disarm_overlay_cancel_mac() {
    use objc2_app_kit::NSEvent;
    OVERLAY_MONITOR.with(|slot| {
        if let Some(monitor) = slot.borrow_mut().take() {
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
    });
}

pub fn place_near_cursor(
    app: &AppHandle,
    label: &str,
    view: &str,
    size: f64,
) -> Result<(), String> {
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

#[cfg(target_os = "windows")]
fn seat_tool_window(window: &tauri::WebviewWindow) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let hwnd = hwnd.0 as windows_sys::Win32::Foundation::HWND;
    unsafe {
        use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
            SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_APPWINDOW,
            WS_EX_TOOLWINDOW,
        };
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let next = (ex | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
        let corner: i32 = 1;
        DwmSetWindowAttribute(hwnd, 33, &corner as *const i32 as *const _, 4);
        let border: u32 = 0xFFFF_FFFE;
        DwmSetWindowAttribute(hwnd, 34, &border as *const u32 as *const _, 4);
    }
}

fn clear_round_window(window: &tauri::WebviewWindow) {
    let _ = window.set_shadow(false);
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    #[cfg(target_os = "windows")]
    seat_tool_window(window);
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
        apply_appearance(app);
        sync_dock(app);
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
        .visible(false)
        .focused(true);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    let window = builder.build().map_err(|err| err.to_string())?;
    watch_window(app, &window);
    list_in_dock(&window);
    seat_traffic_lights(&window);
    apply_appearance(app);
    let _ = window.show();
    let _ = window.set_focus();
    sync_dock(app);
    Ok(())
}

pub fn open_settings(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        apply_appearance(app);
        sync_dock(app);
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
        .visible(false)
        .focused(true);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    let window = builder.build().map_err(|err| err.to_string())?;
    watch_window(app, &window);
    list_in_dock(&window);
    seat_traffic_lights(&window);
    apply_appearance(app);
    let _ = window.show();
    let _ = window.set_focus();
    sync_dock(app);
    Ok(())
}

fn unique_window_title(app: &AppHandle, label: &str, title: &str) -> String {
    let base = if title.trim().is_empty() {
        "Capture"
    } else {
        title.trim()
    };
    let mut candidate = base.to_string();
    let mut n = 2;
    loop {
        let taken = app.webview_windows().into_iter().any(|(other, window)| {
            other != label && window.title().ok().as_deref() == Some(candidate.as_str())
        });
        if !taken {
            return candidate;
        }
        candidate = format!("{base} {n}");
        n += 1;
    }
}

pub fn open_editor(app: &AppHandle, id: &str, title: &str) -> Result<(), String> {
    let label = format!("editor-{id}");
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        list_in_dock(&window);
        apply_appearance(app);
        sync_dock(app);
        return Ok(());
    }
    let offset = app
        .webview_windows()
        .keys()
        .filter(|label| label.starts_with("editor-"))
        .count() as f64;
    let url = WebviewUrl::App("index.html?view=editor".into());
    let builder = WebviewWindowBuilder::new(app, &label, url)
        .title(unique_window_title(app, &label, title))
        .inner_size(1360.0, 828.0)
        .min_inner_size(960.0, 640.0)
        .position(80.0 + offset * 28.0, 48.0 + offset * 28.0)
        .decorations(true)
        .resizable(true)
        .background_color(Color(255, 255, 255, 255))
        .visible(false)
        .focused(true);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    let window = builder.build().map_err(|err| err.to_string())?;
    watch_window(app, &window);
    list_in_dock(&window);
    seat_traffic_lights(&window);
    apply_appearance(app);
    let _ = window.show();
    let _ = window.set_focus();
    sync_dock(app);
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
    let scale = monitor
        .map(|item| item.scale_factor())
        .unwrap_or(1.0)
        .max(1.0);
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
        (
            origin.x + size.width - W - 24.0,
            origin.y + size.height - H - 72.0,
        )
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
