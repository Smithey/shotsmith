mod commands;
mod destinations;
mod features;
mod state;
mod windows;

use state::{app_root, load_presets, load_settings, AppState};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let root = app_root();
    let _ = std::fs::create_dir_all(&root);
    let settings = load_settings(&root);
    let presets = load_presets(&root);
    let history = shotsmith_history::HistoryDb::open(&root.join("history.db")).expect("history database");

    tauri::Builder::default()
        .enable_macos_default_menu(false)
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !features::dispatch_args(app, &args) {
                windows::reveal_documents(app);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        let _ = handle_shortcut(app, shortcut);
                    }
                })
                .build(),
        )
        .manage(AppState {
            root,
            settings: Mutex::new(settings),
            presets: Mutex::new(presets),
            history: Mutex::new(history),
            frozen: Mutex::new(None),
            displays: Mutex::new(Vec::new()),
            last_region: Mutex::new(None),
            editors: Mutex::new(HashMap::new()),
            donut: Mutex::new(None),
            hud: Mutex::new(None),
            capture_mode: Mutex::new("region".into()),
            open_in_id: Mutex::new(None),
        })
        .setup(|app| {
            if let Some(dir) = app.path().app_data_dir().ok() {
                let _ = app.asset_protocol_scope().allow_directory(dir, true);
            }
            let state = app.state::<AppState>();
            let _ = app.asset_protocol_scope().allow_directory(&state.root, true);
            let onboarding = state.settings.lock().map(|s| !s.onboarding_done).unwrap_or(false);
            drop(state);
            let needs_permissions = !shotsmith_platform::screen_recording_granted()
                || !shotsmith_platform::accessibility_granted();
            let _ = rebuild_tray(app.handle());
            let _ = register_shortcuts(app.handle());
            if onboarding || needs_permissions {
                let _ = windows::open_view(app.handle(), "onboarding", "onboarding", "Welcome to Shotsmith", 180.0, 80.0, 520.0, 680.0, true, false);
            }
            windows::sync_dock(app.handle());
            features::spawn_watch(app.handle().clone());
            let args: Vec<String> = std::env::args().collect();
            let _ = features::dispatch_args(app.handle(), &args);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings_get,
            commands::settings_set,
            commands::shortcuts_suspend,
            commands::shortcuts_reload,
            commands::presets_list,
            commands::presets_save,
            commands::presets_delete,
            commands::presets_import,
            commands::presets_export,
            commands::filename_preview,
            commands::permission_status,
            commands::platform_name,
            commands::capture_windows,
            commands::overlay_state,
            commands::capture_begin,
            commands::capture_cancel,
            commands::capture_confirm,
            commands::capture_repeat,
            commands::capture_countdown,
            commands::capture_countdown_done,
            commands::history_list,
            commands::history_delete,
            commands::history_copy,
            commands::editor_open,
            commands::editor_image,
            commands::editor_state,
            commands::editor_commit,
            commands::editor_undo,
            commands::editor_redo,
            commands::editor_crop,
            commands::editor_copy,
            commands::editor_save,
            commands::editor_dirty,
            commands::donut_state,
            commands::hud_state,
            commands::donut_act,
            commands::pin_open,
            commands::pin_close,
            commands::reveal_capture,
            commands::open_history,
            commands::open_settings,
            commands::finish_onboarding,
            commands::open_image,
            commands::edit_clipboard,
            commands::pick_folder,
            commands::pick_save,
            commands::copy_hex,
            commands::quit,
            commands::capture_mode,
            commands::open_screen_settings,
            commands::open_accessibility_settings,
            commands::open_keyboard_settings,
            commands::relaunch,
            commands::feature_notice,
            features::editor_text,
            features::editor_secrets,
            features::editor_dismiss_secret,
            features::editor_redact,
            features::editor_share,
            features::editor_print,
            features::editor_beautify,
            features::apps_list,
            features::open_in_state,
            features::open_in_choose,
            features::measure_at,
            features::lookup_text,
            features::plugins_list,
            features::editor_plugin,
            features::updates_check,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Shotsmith")
        .run(|app, event| {
            match event {
                tauri::RunEvent::ExitRequested { api, code, .. } => {
                    if code.is_none() {
                        api.prevent_exit();
                    }
                }
                tauri::RunEvent::Reopen { .. } => windows::reveal_documents(app),
                _ => {}
            }
        });
}

pub fn register_shortcuts(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().map_err(|err| err.to_string())?.clone();
    drop(state);
    let shortcut = app.global_shortcut();
    shortcut.unregister_all().map_err(|err| err.to_string())?;
    let mut errors = Vec::new();
    for raw in [
        settings.shortcuts.region,
        settings.shortcuts.repeat_last,
        settings.shortcuts.window,
        settings.shortcuts.full_screen,
        settings.shortcuts.history,
    ] {
        let Some(parsed) = parse_shortcut(&raw) else {
            if !raw.trim().is_empty() {
                errors.push(format!("Could not read {raw}"));
            }
            continue;
        };
        if let Err(err) = shortcut.register(parsed) {
            errors.push(err.to_string());
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(". "))
    }
}

fn handle_shortcut(app: &tauri::AppHandle, shortcut: &Shortcut) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().map_err(|err| err.to_string())?.clone();
    drop(state);
    let mode = if same_shortcut(shortcut, &settings.shortcuts.region) {
        "region"
    } else if same_shortcut(shortcut, &settings.shortcuts.repeat_last) {
        "repeat"
    } else if same_shortcut(shortcut, &settings.shortcuts.window) {
        "window"
    } else if same_shortcut(shortcut, &settings.shortcuts.full_screen) {
        "screen"
    } else if same_shortcut(shortcut, &settings.shortcuts.history) {
        "history"
    } else {
        return Ok(());
    };
    let state = app.state::<AppState>();
    match mode {
        "repeat" => {
            commands::capture_repeat(app.clone(), state)?;
        }
        "history" => {
            commands::open_history(app.clone())?;
        }
        "screen" => {
            commands::capture_begin(app.clone(), state, "screen".into())?;
        }
        "window" => {
            commands::capture_begin(app.clone(), state, "window".into())?;
        }
        _ => {
            commands::capture_begin(app.clone(), state, "region".into())?;
        }
    }
    Ok(())
}

pub fn set_tray_capturing(app: &tauri::AppHandle, active: bool) {
    let Some(tray) = app.tray_by_id("shotsmith") else {
        return;
    };
    let bytes: &[u8] = if active {
        include_bytes!("../icons/trayActive.png")
    } else {
        include_bytes!("../icons/trayTemplate.png")
    };
    if let Ok(icon) = Image::from_bytes(bytes) {
        let _ = tray.set_icon(Some(icon));
        let _ = tray.set_icon_as_template(!active);
    }
}

pub fn rebuild_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_menu(build_menu_bar(app)?);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app.remove_menu();
        if let Some(tray) = app.tray_by_id("shotsmith") {
            let _ = tray.set_menu(Some(build_tray_menu(app)?));
            return Ok(());
        }
    }
    if app.tray_by_id("shotsmith").is_some() {
        return Ok(());
    }
    let icon = Image::from_bytes(include_bytes!("../icons/trayTemplate.png"))?;
    let mut builder = TrayIconBuilder::with_id("shotsmith")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Shotsmith")
        .on_menu_event(|app, event| {
            let _ = on_menu(app, event.id.as_ref());
        });
    #[cfg(target_os = "macos")]
    {
        let handle = app.clone();
        builder = builder.on_tray_icon_event(move |_tray, event| {
            if let TrayIconEvent::Click { button_state, rect, .. } = event {
                if button_state == MouseButtonState::Down {
                    windows::toggle_status_menu(&handle, rect);
                }
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let tray_menu = build_tray_menu(app)?;
    #[cfg(not(target_os = "macos"))]
    {
        builder = builder.menu(&tray_menu).show_menu_on_left_click(true);
    }
    builder.build(app)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn build_tray_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    append_capture(&menu, app, true)?;
    Ok(menu)
}

fn chord(tail: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("Cmd+{tail}")
    } else {
        format!("Ctrl+{tail}")
    }
}

#[cfg(target_os = "macos")]
fn build_menu_bar(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let settings_accel = chord(",");
    let settings_item = action(app, "settings", "Settings…", Some(&settings_accel))?;
    let capture = Submenu::with_id(app, "capture-menu", "Capture", true)?;
    append_capture(&capture, app, false)?;
    let window_menu = Submenu::with_id(app, "window-menu", "Window", true)?;
    #[cfg(target_os = "macos")]
    window_menu.set_as_windows_menu_for_nsapp()?;
    let app_menu = Submenu::with_items(
        app,
        "Shotsmith",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("About Shotsmith"), None)?,
            &PredefinedMenuItem::separator(app)?,
            &settings_item,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("Hide Shotsmith"))?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, Some("Quit Shotsmith"))?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &capture, &window_menu])
}

fn append_capture(menu: &impl AppendMenu, app: &tauri::AppHandle, include_app_commands: bool) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().ok();
    let region_accel = settings.as_ref().map(|item| accelerator(&item.shortcuts.region)).unwrap_or_default();
    let repeat_accel = settings.as_ref().map(|item| accelerator(&item.shortcuts.repeat_last)).unwrap_or_default();
    let window_accel = settings.as_ref().map(|item| accelerator(&item.shortcuts.window)).unwrap_or_default();
    let screen_accel = settings.as_ref().map(|item| accelerator(&item.shortcuts.full_screen)).unwrap_or_default();
    let history_accel = settings.as_ref().map(|item| accelerator(&item.shortcuts.history)).unwrap_or_default();
    let region = action(app, "capture-region", "Region", some_accel(&region_accel))?;
    let repeat = action(app, "capture-repeat", "Repeat last region", some_accel(&repeat_accel))?;
    let window = action(app, "capture-window", "Window", some_accel(&window_accel))?;
    let screen = action(app, "capture-screen", "Full screen", some_accel(&screen_accel))?;
    let scrolling = action(app, "capture-scroll", "Scrolling…", None)?;
    let timed_3 = action(app, "timed-3", "3 seconds", None)?;
    let timed_5 = action(app, "timed-5", "5 seconds", None)?;
    let timed_10 = action(app, "timed-10", "10 seconds", None)?;
    let timed = Submenu::with_items(app, "Timed", true, &[&timed_3, &timed_5, &timed_10])?;
    let copy_text = action(app, "capture-text", "Copy text from screen", None)?;
    let measure = action(app, "capture-measure", "Measure", None)?;
    let sep_recent = PredefinedMenuItem::separator(app)?;
    let history = action(app, "history", "Show History", some_accel(&history_accel))?;
    let sep_file = PredefinedMenuItem::separator(app)?;
    let open_accel = chord("O");
    let clip_accel = chord("Shift+V");
    let open_image = action(app, "open-image", "Open image…", Some(&open_accel))?;
    let clipboard = action(app, "clipboard", "Edit image from clipboard", Some(&clip_accel))?;
    let pinned = pinned_menu(app)?;
    menu.append(&region)?;
    menu.append(&repeat)?;
    menu.append(&window)?;
    menu.append(&screen)?;
    menu.append(&scrolling)?;
    menu.append(&timed)?;
    menu.append(&copy_text)?;
    menu.append(&measure)?;
    menu.append(&sep_recent)?;
    menu.append(&history)?;
    menu.append(&sep_file)?;
    menu.append(&open_image)?;
    menu.append(&clipboard)?;
    menu.append(&pinned)?;
    if include_app_commands {
        let sep = PredefinedMenuItem::separator(app)?;
        let settings_accel = chord(",");
        let quit_accel = chord("Q");
        let settings_item = action(app, "settings", "Settings…", Some(&settings_accel))?;
        let quit = action(app, "quit", "Quit Shotsmith", Some(&quit_accel))?;
        menu.append(&sep)?;
        menu.append(&settings_item)?;
        menu.append(&quit)?;
    }
    Ok(())
}

trait AppendMenu {
    fn append(&self, item: &dyn tauri::menu::IsMenuItem<tauri::Wry>) -> tauri::Result<()>;
}

impl AppendMenu for Menu<tauri::Wry> {
    fn append(&self, item: &dyn tauri::menu::IsMenuItem<tauri::Wry>) -> tauri::Result<()> {
        Menu::append(self, item)
    }
}

impl AppendMenu for Submenu<tauri::Wry> {
    fn append(&self, item: &dyn tauri::menu::IsMenuItem<tauri::Wry>) -> tauri::Result<()> {
        Submenu::append(self, item)
    }
}

fn action(app: &tauri::AppHandle, id: &str, label: &str, accelerator: Option<&str>) -> tauri::Result<MenuItem<tauri::Wry>> {
    MenuItem::with_id(app, id, label, true, accelerator)
}

fn some_accel(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}

fn pinned_menu(app: &tauri::AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let state = app.state::<AppState>();
    let pinned = state.history.lock().ok().and_then(|db| db.list(20).ok()).unwrap_or_default();
    let items: Vec<MenuItem<tauri::Wry>> = pinned
        .iter()
        .filter(|record| record.pinned)
        .map(|record| {
            let label = record.window_title.clone().filter(|title| !title.is_empty()).unwrap_or_else(|| "Pinned capture".into());
            MenuItem::with_id(app, format!("pin-{}", record.id), label, true, None::<&str>)
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    if items.is_empty() {
        let empty = action(app, "pin-empty", "No pinned images", None)?;
        empty.set_enabled(false)?;
        return Submenu::with_items(app, "Pinned images", true, &[&empty]);
    }
    let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = items.iter().map(|item| item as _).collect();
    Submenu::with_items(app, "Pinned images", true, &refs)
}

fn on_menu(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    // The menu item runs before AppKit dismisses the menu. Capturing now
    // freezes the open menu into the picture. Let it close first.
    let delayed = matches!(
        id,
        "capture-region" | "capture-window" | "capture-screen" | "capture-repeat" | "capture-scroll" | "capture-text" | "capture-measure"
    ) || id.starts_with("timed-");
    if delayed {
        let app = app.clone();
        let id = id.to_string();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(220));
            let queued = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = run_menu(&queued, &id);
            });
        });
        return Ok(());
    }
    run_menu(app, id)
}

fn run_menu(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    match id {
        "capture-region" => commands::capture_begin(app.clone(), state, "region".into())?,
        "capture-window" => commands::capture_begin(app.clone(), state, "window".into())?,
        "capture-screen" => commands::capture_begin(app.clone(), state, "screen".into())?,
        "capture-repeat" => {
            commands::capture_repeat(app.clone(), state)?;
        }
        "capture-scroll" => commands::capture_begin(app.clone(), state, "scroll".into())?,
        "capture-text" => commands::capture_begin(app.clone(), state, "text".into())?,
        "capture-measure" => commands::capture_begin(app.clone(), state, "measure".into())?,
        "timed-3" | "timed-5" | "timed-10" => {
            if let Ok(seconds) = id.trim_start_matches("timed-").parse::<u32>() {
                if let Ok(mut settings) = state.settings.lock() {
                    settings.timed_seconds = seconds;
                }
            }
            commands::capture_countdown(app.clone(), state)?;
        }
        "history" => commands::open_history(app.clone())?,
        "open-image" => commands::open_image(app.clone(), state)?,
        "clipboard" => commands::edit_clipboard(app.clone(), state)?,
        "settings" => commands::open_settings(app.clone())?,
        "quit" => app.exit(0),
        other if other.starts_with("recent-") && other != "recent-empty" => {
            commands::editor_open(app.clone(), state, other.trim_start_matches("recent-").into())?;
        }
        other if other.starts_with("pin-") && other != "pin-empty" => {
            commands::pin_open(app.clone(), state, other.trim_start_matches("pin-").into())?;
        }
        _ => {}
    }
    Ok(())
}

fn parse_shortcut(raw: &str) -> Option<Shortcut> {
    let raw = raw.trim();
    if raw.is_empty() {
        None
    } else {
        Shortcut::from_str(raw).ok()
    }
}

fn same_shortcut(shortcut: &Shortcut, raw: &str) -> bool {
    parse_shortcut(raw).is_some_and(|parsed| parsed.id() == shortcut.id())
}

fn accelerator(raw: &str) -> String {
    if parse_shortcut(raw).is_some() {
        raw.to_string()
    } else {
        String::new()
    }
}

