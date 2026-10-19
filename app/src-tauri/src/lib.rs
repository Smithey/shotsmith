mod commands;
mod state;
mod windows;

use state::{app_root, load_presets, load_settings, AppState};
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let root = app_root();
    let _ = std::fs::create_dir_all(&root);
    let settings = load_settings(&root);
    let presets = load_presets(&root);
    let history = shotsmith_history::HistoryDb::open(&root.join("history.db")).expect("history database");

    tauri::Builder::default()
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
            editor: Mutex::new(None),
            donut: Mutex::new(None),
            hud: Mutex::new(None),
        })
        .setup(|app| {
            if let Some(dir) = app.path().app_data_dir().ok() {
                let _ = app.asset_protocol_scope().allow_directory(dir, true);
            }
            let state = app.state::<AppState>();
            let _ = app.asset_protocol_scope().allow_directory(&state.root, true);
            let onboarding = state.settings.lock().map(|s| !s.onboarding_done).unwrap_or(false);
            drop(state);
            let _ = rebuild_tray(app.handle());
            let _ = register_shortcuts(app.handle());
            if onboarding {
                let _ = windows::open_view(app.handle(), "onboarding", "onboarding", "Welcome to Shotsmith", 180.0, 120.0, 560.0, 460.0, true, false);
            }
            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings_get,
            commands::settings_set,
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
            commands::editor_open,
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
        ])
        .build(tauri::generate_context!())
        .expect("error while building Shotsmith")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}

pub fn register_shortcuts(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().map_err(|err| err.to_string())?.clone();
    let shortcut = app.global_shortcut();
    let _ = shortcut.unregister_all();
    for raw in [
        settings.shortcuts.region,
        settings.shortcuts.repeat_last,
        settings.shortcuts.window,
        settings.shortcuts.full_screen,
        settings.shortcuts.history,
    ] {
        if let Some(parsed) = parse_shortcut(&raw) {
            let _ = shortcut.register(parsed);
        }
    }
    Ok(())
}

fn handle_shortcut(app: &tauri::AppHandle, shortcut: &Shortcut) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().map_err(|err| err.to_string())?.clone();
    drop(state);
    let raw = shortcut_name(shortcut);
    let mode = if raw == normalize(&settings.shortcuts.region) {
        "region"
    } else if raw == normalize(&settings.shortcuts.repeat_last) {
        "repeat"
    } else if raw == normalize(&settings.shortcuts.window) {
        "window"
    } else if raw == normalize(&settings.shortcuts.full_screen) {
        "screen"
    } else if raw == normalize(&settings.shortcuts.history) {
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

pub fn rebuild_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    if let Some(tray) = app.tray_by_id("shotsmith") {
        tray.set_menu(Some(menu))?;
        return Ok(());
    }
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?;
    TrayIconBuilder::with_id("shotsmith")
        .icon(icon)
        .menu(&menu)
        .tooltip("Shotsmith")
        .on_menu_event(|app, event| {
            let _ = on_menu(app, event.id.as_ref());
        })
        .build(app)?;
    Ok(())
}

fn build_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().ok();
    let region = settings.as_ref().map(|s| shortcut_label(&s.shortcuts.region)).unwrap_or_default();
    let recent = state.history.lock().ok().and_then(|db| db.list(3).ok()).unwrap_or_default();
    let pinned = state.history.lock().ok().and_then(|db| db.list(20).ok()).unwrap_or_default();

    let mut items: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = Vec::new();
    let capture = MenuItem::with_id(app, "capture-region", format!("Region  {region}"), true, None::<&str>)?;
    let repeat = MenuItem::with_id(app, "capture-repeat", "Repeat last region", true, None::<&str>)?;
    let window = MenuItem::with_id(app, "capture-window", "Window", true, None::<&str>)?;
    let screen = MenuItem::with_id(app, "capture-screen", "Full screen", true, None::<&str>)?;
    let timed = MenuItem::with_id(app, "capture-timed", "Timed, 5 s", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let history = MenuItem::with_id(app, "history", "Show History", true, None::<&str>)?;
    let open_image = MenuItem::with_id(app, "open-image", "Open image…", true, None::<&str>)?;
    let clipboard = MenuItem::with_id(app, "clipboard", "Edit image from clipboard", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Shotsmith", true, None::<&str>)?;

    items.extend([
        &capture as &dyn tauri::menu::IsMenuItem<tauri::Wry>,
        &repeat,
        &window,
        &screen,
        &timed,
        &sep1,
    ]);
    let mut recent_items = Vec::new();
    for record in &recent {
        let label = format!("{} · {}×{}", record.date, record.pixel_width, record.pixel_height);
        recent_items.push(MenuItem::with_id(app, format!("recent-{}", record.id), label, true, None::<&str>)?);
    }
    let pinned_items: Vec<MenuItem<tauri::Wry>> = pinned
        .iter()
        .filter(|record| record.pinned)
        .map(|record| MenuItem::with_id(app, format!("pin-{}", record.id), record.window_title.clone().unwrap_or_else(|| record.id.clone()), true, None::<&str>))
        .collect::<tauri::Result<Vec<_>>>()?;
    let pinned_menu = Submenu::with_items(app, "Pinned images", true, &pinned_items.iter().map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>).collect::<Vec<_>>())?;

    let mut menu = Menu::new(app)?;
    menu.append(&capture)?;
    menu.append(&repeat)?;
    menu.append(&window)?;
    menu.append(&screen)?;
    menu.append(&timed)?;
    menu.append(&sep1)?;
    for item in &recent_items {
        menu.append(item)?;
    }
    menu.append(&history)?;
    menu.append(&open_image)?;
    menu.append(&clipboard)?;
    menu.append(&pinned_menu)?;
    menu.append(&sep2)?;
    menu.append(&settings)?;
    menu.append(&quit)?;
    let _ = items;
    Ok(menu)
}

fn on_menu(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    match id {
        "capture-region" => commands::capture_begin(app.clone(), state, "region".into())?,
        "capture-window" => commands::capture_begin(app.clone(), state, "window".into())?,
        "capture-screen" => commands::capture_begin(app.clone(), state, "screen".into())?,
        "capture-repeat" => {
            commands::capture_repeat(app.clone(), state)?;
        }
        "capture-timed" => commands::capture_countdown(app.clone(), state)?,
        "history" => commands::open_history(app.clone())?,
        "open-image" => commands::open_image(app.clone(), state)?,
        "clipboard" => commands::edit_clipboard(app.clone(), state)?,
        "settings" => commands::open_settings(app.clone())?,
        "quit" => app.exit(0),
        other if other.starts_with("recent-") => {
            commands::editor_open(app.clone(), state, other.trim_start_matches("recent-").into())?;
        }
        other if other.starts_with("pin-") => {
            commands::pin_open(app.clone(), state, other.trim_start_matches("pin-").into())?;
        }
        _ => {}
    }
    Ok(())
}

fn parse_shortcut(raw: &str) -> Option<Shortcut> {
    let mut mods = Modifiers::empty();
    let mut code = None;
    for part in raw.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" | "option" => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            "super" | "cmd" | "command" | "meta" | "win" => mods |= Modifiers::SUPER,
            "digit1" | "1" => code = Some(Code::Digit1),
            "digit2" | "2" => code = Some(Code::Digit2),
            "keyy" | "y" => code = Some(Code::KeyY),
            "keyh" | "h" => code = Some(Code::KeyH),
            _ => {}
        }
    }
    Some(Shortcut::new(Some(mods), code?))
}

fn shortcut_name(shortcut: &Shortcut) -> String {
    format!("{shortcut:?}").to_ascii_lowercase()
}

fn normalize(raw: &str) -> String {
    parse_shortcut(raw).map(|shortcut| shortcut_name(&shortcut)).unwrap_or_default()
}

fn shortcut_label(raw: &str) -> String {
    raw.replace("super", "⌘")
        .replace("control", "Ctrl")
        .replace("shift", "Shift")
        .replace("alt", "Alt")
        .replace("digit2", "2")
        .replace("digit1", "1")
        .replace("keyy", "Y")
        .replace("keyh", "H")
        .replace('+', "")
}

