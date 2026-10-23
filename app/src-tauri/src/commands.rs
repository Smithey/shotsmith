use crate::state::{
    default_folder, save_presets, save_settings, AppState, DisplayPayload, DonutPayload,
    DonutSlotPayload, EditorSession, HudPayload, LastRegion,
};
use crate::windows::{
    arm_hud_dismiss, dismiss_donut, hide_labeled, open_view, place_hud, place_near_cursor, show_overlays,
};
use chrono::Local;
use image::RgbaImage;
use shotsmith_capture::PixelRect;
use shotsmith_core::{
    render_pattern, AnnotationObject, Document, ImageFormat, ImageObject, PatternContext, Preset,
    UndoStack,
};
use shotsmith_history::CaptureRecord;
use shotsmith_output::{self, write_atomic};
use shotsmith_platform::{self, PermissionState};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use uuid::Uuid;

fn err_string<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

#[tauri::command]
pub fn settings_get(state: State<AppState>) -> Result<crate::state::Settings, String> {
    Ok(state.settings.lock().map_err(err_string)?.clone())
}

#[tauri::command]
pub fn settings_set(app: AppHandle, state: State<AppState>, settings: crate::state::Settings) -> Result<(), String> {
    {
        let mut current = state.settings.lock().map_err(err_string)?;
        *current = settings.clone();
    }
    save_settings(&state.root, &settings)?;
    apply_autostart(&app, settings.launch_at_login)?;
    crate::register_shortcuts(&app).map_err(err_string)?;
    crate::rebuild_tray(&app).map_err(err_string)?;
    Ok(())
}

#[tauri::command]
pub fn shortcuts_suspend(app: AppHandle) -> Result<(), String> {
    app.global_shortcut().unregister_all().map_err(err_string)
}

#[tauri::command]
pub fn shortcuts_reload(app: AppHandle) -> Result<(), String> {
    crate::register_shortcuts(&app)
}

#[tauri::command]
pub fn presets_list(state: State<AppState>) -> Result<Vec<Preset>, String> {
    Ok(state.presets.lock().map_err(err_string)?.clone())
}

#[tauri::command]
pub fn presets_save(state: State<AppState>, preset: Preset) -> Result<Vec<Preset>, String> {
    let mut presets = state.presets.lock().map_err(err_string)?;
    if let Some(existing) = presets.iter_mut().find(|item| item.id == preset.id) {
        *existing = preset;
    } else {
        presets.push(preset);
    }
    save_presets(&state.root, &presets)?;
    Ok(presets.clone())
}

#[tauri::command]
pub fn presets_delete(state: State<AppState>, id: String) -> Result<Vec<Preset>, String> {
    let mut presets = state.presets.lock().map_err(err_string)?;
    presets.retain(|preset| preset.id != id || preset.builtin);
    save_presets(&state.root, &presets)?;
    Ok(presets.clone())
}

#[tauri::command]
pub fn presets_import(state: State<AppState>, json: String) -> Result<Vec<Preset>, String> {
    let incoming: Vec<Preset> = serde_json::from_str(&json).map_err(err_string)?;
    let mut presets = state.presets.lock().map_err(err_string)?;
    for preset in incoming {
        if let Some(existing) = presets.iter_mut().find(|item| item.id == preset.id) {
            *existing = preset;
        } else {
            presets.push(preset);
        }
    }
    save_presets(&state.root, &presets)?;
    Ok(presets.clone())
}

#[tauri::command]
pub fn presets_export(state: State<AppState>) -> Result<String, String> {
    let presets = state.presets.lock().map_err(err_string)?;
    serde_json::to_string_pretty(&*presets).map_err(err_string)
}

#[tauri::command]
pub fn filename_preview(state: State<AppState>, pattern: String, width: u32, height: u32) -> Result<String, String> {
    let settings = state.settings.lock().map_err(err_string)?;
    Ok(render_pattern(
        &pattern,
        &PatternContext {
            app: "Shotsmith".into(),
            window: "Capture".into(),
            when: Local::now(),
            width,
            height,
            counter: settings.filename_counter,
        },
    ))
}

#[tauri::command]
pub fn permission_status() -> Result<serde_json::Value, String> {
    let screen = shotsmith_platform::screen_recording_granted();
    let accessibility = shotsmith_platform::accessibility_granted();
    Ok(match shotsmith_platform::permission_status() {
        PermissionState::Granted => serde_json::json!({
            "state": "granted",
            "screenRecording": screen,
            "accessibility": accessibility,
        }),
        PermissionState::Denied { reason } => serde_json::json!({
            "state": "denied",
            "reason": reason,
            "screenRecording": screen,
            "accessibility": accessibility,
        }),
    })
}

#[tauri::command]
pub fn platform_name() -> String {
    if cfg!(target_os = "macos") {
        "macos".into()
    } else if cfg!(target_os = "windows") {
        "windows".into()
    } else {
        "linux".into()
    }
}

#[tauri::command]
pub fn capture_windows() -> Result<Vec<shotsmith_platform::WindowInfo>, String> {
    shotsmith_platform::list_windows().map_err(err_string)
}

#[tauri::command]
pub fn overlay_state(state: State<AppState>) -> Result<Vec<DisplayPayload>, String> {
    Ok(state.displays.lock().map_err(err_string)?.clone())
}

#[tauri::command]
pub fn capture_begin(app: AppHandle, state: State<AppState>, mode: String) -> Result<(), String> {
    let next = match mode.as_str() {
        "window" => "window",
        "screen" => "screen",
        _ => "region",
    };
    *state.capture_mode.lock().map_err(err_string)? = next.into();
    match mode.as_str() {
        "screen" => capture_screen(&app, &state),
        "window" | "region" => begin_overlay(&app, &state, true),
        other => Err(format!("Unknown capture mode {other}.")),
    }
}

#[tauri::command]
pub fn capture_mode(state: State<AppState>) -> Result<String, String> {
    Ok(state.capture_mode.lock().map_err(err_string)?.clone())
}

#[tauri::command]
pub fn open_screen_settings() -> Result<(), String> {
    // Open the pane directly. The system prompt and the pane together leave a
    // stale row from the previous build, and the switch then does nothing.
    shotsmith_platform::open_system_settings("screen").map_err(err_string)
}

#[tauri::command]
pub fn open_keyboard_settings() -> Result<(), String> {
    shotsmith_platform::open_system_settings("keyboard").map_err(err_string)
}

#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    shotsmith_platform::open_system_settings("accessibility").map_err(err_string)
}

#[tauri::command]
pub fn relaunch(app: AppHandle) {
    app.restart();
}

#[tauri::command]
pub fn capture_cancel(app: AppHandle) -> Result<(), String> {
    hide_on_main(&app, "overlay-")
}

#[tauri::command]
pub fn capture_confirm(
    app: AppHandle,
    state: State<AppState>,
    display_id: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    window_id: Option<u32>,
    source_app: Option<String>,
    window_title: Option<String>,
    include_cursor: bool,
    cursor_x: f32,
    cursor_y: f32,
    mode: String,
) -> Result<String, String> {
    let image = if let Some(id) = window_id {
        shotsmith_platform::capture_window(id).or_else(|_| crop_frozen(&state, display_id, x, y, width, height))?
    } else {
        crop_frozen(&state, display_id, x, y, width, height)?
    };
    {
        let mut last = state.last_region.lock().map_err(err_string)?;
        *last = Some(LastRegion { display_id, x, y, width, height });
    }
    hide_on_main(&app, "overlay-")?;
    let scale = display_scale(&state, display_id);
    store_and_route(&app, &state, image, &mode, source_app, window_title, include_cursor, cursor_x, cursor_y, scale)
}

#[tauri::command]
pub fn capture_repeat(app: AppHandle, state: State<AppState>) -> Result<String, String> {
    let last = state.last_region.lock().map_err(err_string)?.clone();
    let Some(last) = last else {
        return Err("There is no previous region yet.".into());
    };
    let _ = begin_overlay(&app, &state, false);
    let image = crop_frozen(&state, last.display_id, last.x, last.y, last.width, last.height)?;
    hide_on_main(&app, "overlay-")?;
    let settings = state.settings.lock().map_err(err_string)?.clone();
    let scale = display_scale(&state, last.display_id);
    store_and_route(
        &app,
        &state,
        image,
        "region",
        Some("Screen".into()),
        Some("Repeat".into()),
        settings.include_cursor,
        24.0,
        24.0,
        scale,
    )
}

#[tauri::command]
pub fn capture_countdown(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let settings = state.settings.lock().map_err(err_string)?.clone();
    let hud = HudPayload {
        title: format!("Capturing in {}", settings.timed_seconds),
        detail: if settings.timed_mode == "screen" { "Full screen".into() } else { "Then pick a region".into() },
        image_path: String::new(),
        capture_id: String::new(),
        countdown: Some(settings.timed_seconds),
    };
    *state.hud.lock().map_err(err_string)? = Some(hud);
    place_hud(&app)
}

#[tauri::command]
pub fn capture_countdown_done(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let mode = state.settings.lock().map_err(err_string)?.timed_mode.clone();
    if let Some(window) = app.get_webview_window("hud") {
        let _ = window.hide();
    }
    if mode == "screen" {
        capture_screen(&app, &state)
    } else {
        begin_overlay(&app, &state, true)
    }
}

#[tauri::command]
pub fn history_list(state: State<AppState>, query: String) -> Result<Vec<CaptureRecord>, String> {
    let history = state.history.lock().map_err(err_string)?;
    if query.trim().is_empty() {
        history.list(200).map_err(err_string)
    } else {
        history.search(query.trim()).map_err(err_string)
    }
}

#[tauri::command]
pub fn history_delete(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let removed = {
        let history = state.history.lock().map_err(err_string)?;
        history.delete(&id).map_err(err_string)?
    };
    if let Some(record) = removed {
        if let Some(parent) = PathBuf::from(&record.original_path).parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
    crate::rebuild_tray(&app).map_err(err_string)?;
    Ok(())
}

#[tauri::command]
pub fn editor_open(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let record = state.history.lock().map_err(err_string)?.get(&id).map_err(err_string)?;
    let Some(record) = record else {
        return Err("That capture is no longer in History.".into());
    };
    let (document, _) = if let Some(path) = &record.document_path {
        shotsmith_output::read_shotsmith(std::path::Path::new(path)).map_err(err_string)?
    } else {
        let image = image::open(&record.original_path).map_err(err_string)?.to_rgba8();
        (Document::new(image.width(), image.height()), Vec::new())
    };
    *state.editor.lock().map_err(err_string)? = Some(EditorSession {
        capture_id: id,
        stack: UndoStack::new(document),
        image_path: record.original_path,
        dirty: false,
    });
    // Close the wheel first, then open the editor on a later turn. Creating
    // the editor window inside the wheel's click aborts WebKit and leaves
    // the wheel on screen.
    let queued = app.clone();
    app.run_on_main_thread(move || {
        dismiss_donut(&queued);
        let open = queued.clone();
        let _ = queued.run_on_main_thread(move || {
            let _ = crate::windows::open_editor(&open);
        });
    })
    .map_err(err_string)
}

#[tauri::command]
pub fn editor_state(state: State<AppState>) -> Result<serde_json::Value, String> {
    let editor = state.editor.lock().map_err(err_string)?;
    let Some(session) = editor.as_ref() else {
        return Err("No capture is open.".into());
    };
    Ok(serde_json::json!({
        "captureId": session.capture_id,
        "document": session.stack.current(),
        "imagePath": session.image_path,
        "canUndo": session.stack.can_undo(),
        "canRedo": session.stack.can_redo(),
        "dirty": session.dirty,
    }))
}

#[tauri::command]
pub fn editor_commit(state: State<AppState>, document: Document) -> Result<serde_json::Value, String> {
    let mut editor = state.editor.lock().map_err(err_string)?;
    let Some(session) = editor.as_mut() else {
        return Err("No capture is open.".into());
    };
    session.stack.commit(document);
    session.dirty = true;
    Ok(serde_json::json!({
        "document": session.stack.current(),
        "canUndo": session.stack.can_undo(),
        "canRedo": session.stack.can_redo(),
    }))
}

#[tauri::command]
pub fn editor_undo(state: State<AppState>) -> Result<serde_json::Value, String> {
    let mut editor = state.editor.lock().map_err(err_string)?;
    let Some(session) = editor.as_mut() else {
        return Err("No capture is open.".into());
    };
    session.stack.undo();
    session.dirty = true;
    Ok(serde_json::json!({
        "document": session.stack.current(),
        "canUndo": session.stack.can_undo(),
        "canRedo": session.stack.can_redo(),
    }))
}

#[tauri::command]
pub fn editor_redo(state: State<AppState>) -> Result<serde_json::Value, String> {
    let mut editor = state.editor.lock().map_err(err_string)?;
    let Some(session) = editor.as_mut() else {
        return Err("No capture is open.".into());
    };
    session.stack.redo();
    session.dirty = true;
    Ok(serde_json::json!({
        "document": session.stack.current(),
        "canUndo": session.stack.can_undo(),
        "canRedo": session.stack.can_redo(),
    }))
}

#[tauri::command]
pub fn editor_crop(state: State<AppState>, x: u32, y: u32, width: u32, height: u32) -> Result<serde_json::Value, String> {
    let mut editor = state.editor.lock().map_err(err_string)?;
    let Some(session) = editor.as_mut() else {
        return Err("No capture is open.".into());
    };
    let image = image::open(&session.image_path).map_err(err_string)?.to_rgba8();
    let cropped = shotsmith_capture::crop(&image, PixelRect { x: x as i32, y: y as i32, width, height });
    let png = shotsmith_output::encode(&cropped, ImageFormat::Png, 1.0).map_err(err_string)?;
    write_atomic(std::path::Path::new(&session.image_path), &png).map_err(err_string)?;
    let mut document = session.stack.current().clone();
    shotsmith_core::crop_document(&mut document, x as f32, y as f32, cropped.width() as f32, cropped.height() as f32);
    session.stack.commit(document);
    session.dirty = true;
    Ok(serde_json::json!({
        "document": session.stack.current(),
        "imagePath": session.image_path,
        "canUndo": session.stack.can_undo(),
        "canRedo": session.stack.can_redo(),
    }))
}

#[tauri::command]
pub fn editor_copy(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let image = {
        let editor = state.editor.lock().map_err(err_string)?;
        let Some(session) = editor.as_ref() else {
            return Err("No capture is open.".into());
        };
        flattened(session)?
    };
    // Clipboard access has to run on the main thread. Calling it from this
    // command aborts the process while the editor webview is still answering.
    app.run_on_main_thread(move || {
        let _ = copy_image(&image);
    })
    .map_err(err_string)
}

#[tauri::command]
pub fn editor_save(app: AppHandle, state: State<AppState>, preset_id: Option<String>, save_as: Option<String>) -> Result<String, String> {
    let (capture_id, image, document, image_path) = {
        let editor = state.editor.lock().map_err(err_string)?;
        let Some(session) = editor.as_ref() else {
            return Err("No capture is open.".into());
        };
        (
            session.capture_id.clone(),
            flattened(session)?,
            session.stack.current().clone(),
            session.image_path.clone(),
        )
    };
    let scale = capture_scale(&state, &capture_id);
    let mut clipboard_image = None;
    let path = if let Some(requested) = save_as {
        let path = PathBuf::from(&requested);
        let format = format_from_path(&path);
        let bytes = shotsmith_output::encode(&image, format, 0.9).map_err(err_string)?;
        write_atomic(&path, &bytes).map_err(err_string)?;
        path
    } else {
        let preset = preset_by_id(&state, preset_id)?;
        let wants_copy = preset.folder.is_none()
            || preset.after_save.contains(&shotsmith_core::AfterSaveAction::CopyImage);
        if wants_copy {
            clipboard_image = Some(image.clone());
        }
        export_with_preset(&state, &image, &preset, scale, false)?
    };
    persist_document(&state, &capture_id, &document)?;
    if let Ok(mut editor) = state.editor.lock() {
        if let Some(session) = editor.as_mut() {
            session.dirty = false;
        }
    }
    let detail = path.to_string_lossy().to_string();
    let hud_detail = detail.clone();
    let hud_id = capture_id.clone();
    let hud_image = image_path.clone();
    let app_for_ui = app.clone();
    app.run_on_main_thread(move || {
        if let Some(image) = clipboard_image {
            let _ = copy_image(&image);
        }
        let state = app_for_ui.state::<AppState>();
        let _ = show_hud(&app_for_ui, &state, "Saved", &hud_detail, &hud_id, &hud_image);
    })
    .map_err(err_string)?;
    Ok(detail)
}

#[tauri::command]
pub fn editor_dirty(state: State<AppState>) -> Result<bool, String> {
    Ok(state.editor.lock().map_err(err_string)?.as_ref().map(|s| s.dirty).unwrap_or(false))
}

#[tauri::command]
pub fn donut_state(state: State<AppState>) -> Result<DonutPayload, String> {
    state.donut.lock().map_err(err_string)?.clone().ok_or_else(|| "The donut is not open.".into())
}

#[tauri::command]
pub fn hud_state(state: State<AppState>) -> Result<HudPayload, String> {
    state.hud.lock().map_err(err_string)?.clone().ok_or_else(|| "No HUD is open.".into())
}

#[tauri::command]
pub fn donut_act(app: AppHandle, state: State<AppState>, action: String, keep_open: bool) -> Result<(), String> {
    let payload = state.donut.lock().map_err(err_string)?.clone();
    let Some(payload) = payload else {
        return Err("The donut is not open.".into());
    };
    // Window and clipboard calls have to run on the main thread. Doing them
    // inside this command destroys the webview while it is still answering
    // the click, which aborts the process.
    app.clone().run_on_main_thread(move || {
        let state = app.state::<AppState>();
        if !keep_open {
            if let Ok(mut donut) = state.donut.lock() {
                *donut = None;
            }
            hide_labeled(&app, "donut");
        }
        if let Err(err) = run_action(&app, &state, &payload.capture_id, &action) {
            let _ = show_hud(
                &app,
                &state,
                "Could not finish",
                &err,
                &payload.capture_id,
                &payload.image_path,
            );
        }
    })
    .map_err(err_string)
}

#[tauri::command]
pub fn pin_open(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    state.history.lock().map_err(err_string)?.set_pinned(&id, true).map_err(err_string)?;
    let (x, y) = shotsmith_platform::cursor_position();
    open_view(&app, &format!("pin-{id}"), &format!("pin&id={id}"), "Pin", x as f64, y as f64, 360.0, 240.0, false, false)?;
    crate::rebuild_tray(&app).map_err(err_string)
}

#[tauri::command]
pub fn pin_close(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&format!("pin-{id}")) {
        let _ = window.close();
    }
    Ok(())
}

#[tauri::command]
pub fn reveal_capture(state: State<AppState>, id: String) -> Result<(), String> {
    let record = state.history.lock().map_err(err_string)?.get(&id).map_err(err_string)?;
    let Some(record) = record else {
        return Err("That capture is no longer in History.".into());
    };
    shotsmith_platform::reveal_path(std::path::Path::new(&record.original_path)).map_err(err_string)
}

#[tauri::command]
pub fn open_history(app: AppHandle) -> Result<(), String> {
    crate::windows::open_history(&app)
}

#[tauri::command]
pub fn history_copy(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let record = state.history.lock().map_err(err_string)?.get(&id).map_err(err_string)?;
    let Some(record) = record else {
        return Err("That capture is no longer in History.".into());
    };
    let image = image::open(&record.original_path).map_err(err_string)?.to_rgba8();
    let path = record.original_path.clone();
    let capture_id = id.clone();
    let app_for_ui = app.clone();
    app.run_on_main_thread(move || {
        let _ = copy_image(&image);
        let state = app_for_ui.state::<AppState>();
        let _ = show_hud(&app_for_ui, &state, "Copied to clipboard", "From History", &capture_id, &path);
    })
    .map_err(err_string)
}

#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    crate::windows::open_settings(&app)
}

#[tauri::command]
pub fn feature_notice(app: AppHandle, message: String) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
    app.dialog().message(message).kind(MessageDialogKind::Info).show(|_| {});
}

#[tauri::command]
pub fn finish_onboarding(app: AppHandle, state: State<AppState>, after_capture: String) -> Result<(), String> {
    let mut settings = state.settings.lock().map_err(err_string)?.clone();
    settings.onboarding_done = true;
    settings.after_capture = after_capture;
    drop(state.settings.lock().map_err(err_string)?);
    {
        let mut current = state.settings.lock().map_err(err_string)?;
        *current = settings.clone();
    }
    save_settings(&state.root, &settings)?;
    if let Some(window) = app.get_webview_window("onboarding") {
        let _ = window.close();
    }
    Ok(())
}

#[tauri::command]
pub fn open_image(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;
    // The blocking picker runs the panel on the main thread and waits there.
    // The menu calls this on that thread, so a wait here would deadlock.
    drop(state);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut builder = app
        .dialog()
        .file()
        .set_title("Open image")
        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif", "tif", "tiff", "heic", "bmp"]);
    if let Some(window) = app
        .get_webview_window("editor")
        .or_else(|| app.get_webview_window("history"))
        .or_else(|| app.get_webview_window("settings"))
    {
        builder = builder.set_parent(&window);
    }
    builder.pick_file(move |path| {
        let _ = tx.send(path);
    });
    let finish = move |app: AppHandle| -> Result<(), String> {
        let Some(path) = rx.recv().map_err(err_string)? else {
            return Ok(());
        };
        let path = path.into_path().map_err(|err| err.to_string())?;
        let image = image::open(&path).map_err(err_string)?.to_rgba8();
        let title = path.file_name().map(|name| name.to_string_lossy().to_string());
        let opened = app.clone();
        let state = app.state::<AppState>();
        let id = store_and_route(&app, &state, image, "region", Some("File".into()), title, false, 0.0, 0.0, 1.0)?;
        editor_open(opened, state, id)
    };
    if on_main_thread() {
        std::thread::spawn(move || {
            let _ = finish(app);
        });
        Ok(())
    } else {
        finish(app)
    }
}

#[tauri::command]
pub fn edit_clipboard(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(err_string)?;
    let image = clipboard.get_image().map_err(err_string)?;
    let buffer = RgbaImage::from_raw(image.width as u32, image.height as u32, image.bytes.into_owned())
        .ok_or_else(|| "The clipboard image could not be read.".to_string())?;
    let id = store_and_route(&app, &state, buffer, "region", Some("Clipboard".into()), None, false, 0.0, 0.0, 1.0)?;
    editor_open(app, state, id)
}

#[tauri::command]
pub fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    // The blocking picker runs the panel on the main thread and waits there,
    // which aborts the settings webview. Parent the panel and wait off that thread.
    let (tx, rx) = std::sync::mpsc::channel();
    let mut builder = app.dialog().file().set_title("Save location");
    if let Some(window) = app.get_webview_window("settings") {
        builder = builder.set_parent(&window);
    }
    builder.pick_folder(move |path| {
        let _ = tx.send(path);
    });
    let Some(path) = rx.recv().map_err(err_string)? else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|err| err.to_string())?;
    Ok(Some(path.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn pick_save(app: AppHandle, suggested: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    // Parent the sheet to the editor. The blocking helper runs the panel on
    // the main thread and then waits there, which freezes the webview.
    let (tx, rx) = std::sync::mpsc::channel();
    let mut builder = app.dialog().file().set_title("Save capture").set_file_name(&suggested);
    if let Some(window) = app.get_webview_window("editor") {
        builder = builder.set_parent(&window);
    }
    builder.save_file(move |path| {
        let _ = tx.send(path);
    });
    let Some(path) = rx.recv().map_err(err_string)? else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|err| err.to_string())?;
    Ok(Some(path.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn copy_hex(text: String) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(err_string)?;
    clipboard.set_text(text).map_err(err_string)
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

fn on_main_thread() -> bool {
    #[cfg(target_os = "macos")]
    {
        objc2::MainThreadMarker::new().is_some()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

fn hide_on_main(app: &AppHandle, prefix: &str) -> Result<(), String> {
    if on_main_thread() {
        hide_labeled(app, prefix);
        return Ok(());
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let queued = app.clone();
    let prefix = prefix.to_string();
    app.run_on_main_thread(move || {
        hide_labeled(&queued, &prefix);
        let _ = tx.send(());
    })
    .map_err(err_string)?;
    rx.recv_timeout(std::time::Duration::from_secs(2)).map_err(|_| "The window did not close.".to_string())
}

fn begin_overlay(app: &AppHandle, state: &State<AppState>, present: bool) -> Result<(), String> {
    let status = shotsmith_platform::permission_status();
    if let PermissionState::Denied { reason } = status {
        let queued = app.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = open_view(
                &queued,
                "onboarding",
                "onboarding",
                "Allow screen capture",
                180.0,
                120.0,
                520.0,
                680.0,
                true,
                false,
            );
        });
        return Err(format!("Screen capture is blocked. {reason}"));
    }
    // Hide the previous overlay before the snapshot, or the next capture is a
    // picture of the overlay itself.
    hide_on_main(app, "overlay-")?;
    let frozen = shotsmith_platform::freeze_all().map_err(err_string)?;
    let mut payloads = Vec::new();
    let dir = state.root.join("freeze");
    std::fs::create_dir_all(&dir).map_err(err_string)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    for display in &frozen {
        let path = dir.join(format!("{}-{stamp}.png", display.info.id));
        let png = shotsmith_output::encode(&display.image, ImageFormat::Png, 1.0).map_err(err_string)?;
        write_atomic(&path, &png).map_err(err_string)?;
        payloads.push(DisplayPayload {
            id: display.info.id,
            name: display.info.name.clone(),
            x: display.info.x,
            y: display.info.y,
            width: display.image.width(),
            height: display.image.height(),
            scale: display.info.scale,
            image_path: path.to_string_lossy().to_string(),
        });
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".png") && !name.contains(&stamp.to_string()) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    *state.frozen.lock().map_err(err_string)? = Some(frozen);
    *state.displays.lock().map_err(err_string)? = payloads;
    if !present {
        return Ok(());
    }
    if on_main_thread() {
        return show_overlays(app);
    }
    let queued = app.clone();
    app.run_on_main_thread(move || {
        let _ = show_overlays(&queued);
    })
    .map_err(err_string)
}

fn capture_screen(app: &AppHandle, state: &State<AppState>) -> Result<(), String> {
    let _ = begin_overlay(app, state, false)?;
    let (cx, cy) = shotsmith_platform::cursor_position();
    let displays = state.displays.lock().map_err(err_string)?.clone();
    let display = displays
        .iter()
        .find(|display| cx >= display.x && cy >= display.y && cx < display.x + display.width as i32 && cy < display.y + display.height as i32)
        .cloned()
        .or_else(|| displays.first().cloned())
        .ok_or_else(|| "No display to capture.".to_string())?;
    hide_on_main(app, "overlay-")?;
    let image = crop_frozen(state, display.id, 0, 0, display.width, display.height)?;
    store_and_route(app, state, image, "screen", Some(display.name), None, false, 0.0, 0.0, display.scale).map(|_| ())
}

fn crop_frozen(state: &State<AppState>, display_id: u32, x: u32, y: u32, width: u32, height: u32) -> Result<RgbaImage, String> {
    let frozen = state.frozen.lock().map_err(err_string)?;
    let Some(frozen) = frozen.as_ref() else {
        return Err("The screen is no longer frozen. Start the capture again.".into());
    };
    let display = frozen.iter().find(|item| item.info.id == display_id).or(frozen.first());
    let Some(display) = display else {
        return Err("That display is not available.".into());
    };
    Ok(shotsmith_capture::crop(&display.image, PixelRect { x: x as i32, y: y as i32, width, height }))
}

fn store_and_route(
    app: &AppHandle,
    state: &State<AppState>,
    image: RgbaImage,
    mode: &str,
    source_app: Option<String>,
    window_title: Option<String>,
    include_cursor: bool,
    cursor_x: f32,
    cursor_y: f32,
    scale: f32,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let dir = state.root.join("History").join(&id);
    std::fs::create_dir_all(&dir).map_err(err_string)?;
    let mut document = Document::new(image.width(), image.height());
    if include_cursor {
        document.objects.push(AnnotationObject::Cursor(ImageObject {
            id: Uuid::new_v4().to_string(),
            x: cursor_x,
            y: cursor_y,
            w: 24.0,
            h: 24.0,
            png_base64: shotsmith_render::cursor_png(),
        }));
    }
    let original = shotsmith_output::encode(&image, ImageFormat::Png, 1.0).map_err(err_string)?;
    let original_path = dir.join("original.png");
    write_atomic(&original_path, &original).map_err(err_string)?;
    let document_path = dir.join("capture.shotsmith");
    shotsmith_output::write_shotsmith(&document_path, &document, &original).map_err(err_string)?;
    let record = CaptureRecord {
        id: id.clone(),
        date: chrono::Utc::now().to_rfc3339(),
        mode: mode.into(),
        source_app: source_app.clone(),
        window_title: window_title.clone(),
        pixel_width: image.width(),
        pixel_height: image.height(),
        scale,
        original_path: original_path.to_string_lossy().to_string(),
        document_path: Some(document_path.to_string_lossy().to_string()),
        pinned: false,
    };
    {
        let history = state.history.lock().map_err(err_string)?;
        history.insert(&record).map_err(err_string)?;
        let settings = state.settings.lock().map_err(err_string)?.clone();
        if settings.keep_history {
            let removed = history.prune(settings.history_limit, settings.history_days).map_err(err_string)?;
            for old in removed {
                if let Some(parent) = PathBuf::from(old.original_path).parent() {
                    let _ = std::fs::remove_dir_all(parent);
                }
            }
        }
    }
    let settings = state.settings.lock().map_err(err_string)?.clone();
    let flat = shotsmith_render::render(&image, &document, &[]);
    let _ = shotsmith_extensions::install_builtin();
    match settings.after_capture.as_str() {
        "copy" => {
            copy_image(&flat)?;
            show_hud(app, state, "Copied to clipboard", "Also saved to History", &id, &original_path.to_string_lossy())?;
        }
        "save" => {
            let preset = preset_by_id(state, None)?;
            let saved = export_with_preset(state, &flat, &preset, scale, true)?;
            show_hud(app, state, "Saved", &saved.to_string_lossy(), &id, &original_path.to_string_lossy())?;
        }
        "editor" => {
            editor_open(app.clone(), state.clone(), id.clone())?;
        }
        _ => {
            let slots = donut_slots(state)?;
            *state.donut.lock().map_err(err_string)? = Some(DonutPayload {
                capture_id: id.clone(),
                width: image.width(),
                height: image.height(),
                image_path: original_path.to_string_lossy().to_string(),
                preview: thumbnail_data_url(&image),
                slots,
                cursor_x: shotsmith_platform::cursor_position().0,
                cursor_y: shotsmith_platform::cursor_position().1,
            });
            let queued = app.clone();
            app.run_on_main_thread(move || {
                let _ = place_near_cursor(&queued, "donut", "donut", 300.0);
            })
            .map_err(err_string)?;
        }
    }
    crate::rebuild_tray(app).map_err(err_string)?;
    let _ = (source_app, window_title);
    Ok(id)
}

fn donut_slots(state: &State<AppState>) -> Result<Vec<DonutSlotPayload>, String> {
    let settings = state.settings.lock().map_err(err_string)?;
    let presets = state.presets.lock().map_err(err_string)?;
    let count = settings.donut_count.clamp(4, 8) as usize;
    let mut slots = Vec::new();
    for (index, action) in settings.donut_slots.iter().take(count).enumerate() {
        let label = if let Some(id) = action.strip_prefix("preset:") {
            presets.iter().find(|preset| preset.id == id).map(|preset| preset.name.clone()).unwrap_or_else(|| "Preset".into())
        } else {
            match action.as_str() {
                "copy" => "Copy",
                "save" => "Save",
                "annotate" => "Annotate",
                "pin" => "Pin",
                "share" => "Share",
                "copy-text" => "Copy text",
                "open-in" => "Open in…",
                _ => "Action",
            }
            .into()
        };
        slots.push(DonutSlotPayload {
            id: action.clone(),
            label,
            key: (index + 1).to_string(),
        });
    }
    Ok(slots)
}

fn run_action(app: &AppHandle, state: &State<AppState>, id: &str, action: &str) -> Result<(), String> {
    let record = state.history.lock().map_err(err_string)?.get(id).map_err(err_string)?;
    let Some(record) = record else {
        return Err("That capture is no longer in History.".into());
    };
    let image = image::open(&record.original_path).map_err(err_string)?.to_rgba8();
    let flat = with_annotations(&record, &image);
    match action {
        "copy" => {
            copy_image(&flat)?;
            show_hud(app, state, "Copied to clipboard", "Also saved to History", id, &record.original_path)
        }
        "save" => {
            let preset = preset_by_id(state, None)?;
            let saved = export_with_preset(state, &flat, &preset, record.scale, true)?;
            show_hud(app, state, "Saved", &saved.to_string_lossy(), id, &record.original_path)
        }
        "annotate" => editor_open(app.clone(), state.clone(), id.to_string()),
        "pin" => pin_open(app.clone(), state.clone(), id.to_string()),
        "share" => {
            copy_image(&flat)?;
            show_hud(app, state, "Copied to clipboard", "Paste it where you want to share", id, &record.original_path)
        }
        "copy-text" => show_hud(
            app,
            state,
            "Copy text is not ready",
            "Text from a capture arrives in a later version",
            id,
            &record.original_path,
        ),
        "open-in" => {
            shotsmith_platform::reveal_path(std::path::Path::new(&record.original_path)).map_err(err_string)?;
            show_hud(app, state, "Shown in Finder", "Open it with another app from there", id, &record.original_path)
        }
        other if other.starts_with("preset:") => {
            let preset = preset_by_id(state, Some(other.trim_start_matches("preset:").into()))?;
            let saved = export_with_preset(state, &flat, &preset, record.scale, true)?;
            show_hud(app, state, &format!("Saved with {}", preset.name), &saved.to_string_lossy(), id, &record.original_path)
        }
        other => Err(format!("Unknown action {other}.")),
    }
}

fn preset_by_id(state: &State<AppState>, id: Option<String>) -> Result<Preset, String> {
    let settings = state.settings.lock().map_err(err_string)?;
    let wanted = id.unwrap_or_else(|| settings.default_preset_id.clone());
    let presets = state.presets.lock().map_err(err_string)?;
    presets.iter().find(|preset| preset.id == wanted).cloned().ok_or_else(|| "That preset is missing.".into())
}

fn export_with_preset(state: &State<AppState>, image: &RgbaImage, preset: &Preset, scale: f32, use_clipboard: bool) -> Result<PathBuf, String> {
    let Some(folder) = preset.folder.clone() else {
        if use_clipboard {
            copy_image(image)?;
        }
        return Ok(PathBuf::from("clipboard"));
    };
    let settings = state.settings.lock().map_err(err_string)?.clone();
    let name = render_pattern(
        &preset.filename_pattern,
        &PatternContext {
            app: "Shotsmith".into(),
            window: "Capture".into(),
            when: Local::now(),
            width: image.width(),
            height: image.height(),
            counter: settings.filename_counter,
        },
    );
    let rendered = shotsmith_render::render(image, &Document::new(image.width(), image.height()), &preset.effects);
    let scaled = scale_output(&rendered, preset, scale);
    let bytes = shotsmith_output::encode(&scaled, preset.format, preset.quality).map_err(err_string)?;
    let path = shotsmith_core::collision_path(
        &PathBuf::from(&folder).join(format!("{name}.{}", preset.format.extension())),
        |candidate| candidate.exists(),
    );
    write_atomic(&path, &bytes).map_err(err_string)?;
    {
        let mut settings = state.settings.lock().map_err(err_string)?;
        settings.filename_counter += 1;
        let snapshot = settings.clone();
        drop(settings);
        let _ = save_settings(&state.root, &snapshot);
    }
    for action in &preset.after_save {
        match action {
            shotsmith_core::AfterSaveAction::CopyImage => {
                if use_clipboard {
                    let _ = copy_image(&scaled);
                }
            }
            shotsmith_core::AfterSaveAction::CopyPath => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(path.to_string_lossy().to_string());
                }
            }
            shotsmith_core::AfterSaveAction::Reveal => {
                let _ = shotsmith_platform::reveal_path(&path);
            }
            shotsmith_core::AfterSaveAction::OpenEditor => {}
        }
    }
    let _ = default_folder();
    Ok(path)
}

fn scale_output(image: &RgbaImage, preset: &Preset, scale: f32) -> RgbaImage {
    if preset.scale != shotsmith_core::OutputScale::Points || scale <= 1.01 {
        return image.clone();
    }
    let width = ((image.width() as f32) / scale).round().max(1.0) as u32;
    let height = ((image.height() as f32) / scale).round().max(1.0) as u32;
    if width == image.width() && height == image.height() {
        return image.clone();
    }
    image::imageops::resize(image, width, height, image::imageops::FilterType::Triangle)
}

fn display_scale(state: &State<AppState>, display_id: u32) -> f32 {
    state
        .displays
        .lock()
        .ok()
        .and_then(|displays| displays.iter().find(|display| display.id == display_id).map(|display| display.scale))
        .unwrap_or(1.0)
}

fn capture_scale(state: &State<AppState>, id: &str) -> f32 {
    state
        .history
        .lock()
        .ok()
        .and_then(|history| history.get(id).ok().flatten())
        .map(|record| record.scale)
        .unwrap_or(1.0)
}

fn with_annotations(record: &CaptureRecord, image: &RgbaImage) -> RgbaImage {
    let Some(path) = &record.document_path else {
        return image.clone();
    };
    let Ok((document, _)) = shotsmith_output::read_shotsmith(std::path::Path::new(path)) else {
        return image.clone();
    };
    shotsmith_render::render(image, &document, &[])
}

fn flattened(session: &EditorSession) -> Result<RgbaImage, String> {
    let image = image::open(&session.image_path).map_err(err_string)?.to_rgba8();
    let mut document = session.stack.current().clone();
    shotsmith_extensions::install_builtin().before_export(&mut document);
    Ok(shotsmith_render::render(&image, &document, &[]))
}

fn persist_document(state: &State<AppState>, id: &str, document: &Document) -> Result<(), String> {
    let record = state.history.lock().map_err(err_string)?.get(id).map_err(err_string)?;
    let Some(record) = record else {
        return Ok(());
    };
    let path = record.document_path.unwrap_or_else(|| {
        PathBuf::from(&record.original_path).with_file_name("capture.shotsmith").to_string_lossy().to_string()
    });
    let original = std::fs::read(&record.original_path).unwrap_or_default();
    shotsmith_output::write_shotsmith(std::path::Path::new(&path), document, &original).map_err(err_string)?;
    state.history.lock().map_err(err_string)?.set_document(id, &path).map_err(err_string)
}

fn thumbnail_data_url(image: &RgbaImage) -> String {
    let longest = image.width().max(image.height()).max(1) as f32;
    let scale = (240.0 / longest).min(1.0);
    let width = (image.width() as f32 * scale).round().max(1.0) as u32;
    let height = (image.height() as f32 * scale).round().max(1.0) as u32;
    let thumb = image::imageops::resize(image, width, height, image::imageops::FilterType::Triangle);
    let mut png = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png);
    if image::ImageEncoder::write_image(
        encoder,
        thumb.as_raw(),
        thumb.width(),
        thumb.height(),
        image::ExtendedColorType::Rgba8,
    )
    .is_err()
    {
        return String::new();
    }
    format!(
        "data:image/png;base64,{}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png)
    )
}

fn copy_image(image: &RgbaImage) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(err_string)?;
    clipboard
        .set_image(arboard::ImageData {
            width: image.width() as usize,
            height: image.height() as usize,
            bytes: std::borrow::Cow::Borrowed(image.as_raw()),
        })
        .map_err(err_string)
}

fn show_hud(app: &AppHandle, state: &State<AppState>, title: &str, detail: &str, id: &str, image_path: &str) -> Result<(), String> {
    let enabled = state.settings.lock().map_err(err_string)?.hud_enabled;
    if !enabled {
        return Ok(());
    }
    *state.hud.lock().map_err(err_string)? = Some(HudPayload {
        title: title.into(),
        detail: detail.into(),
        image_path: image_path.into(),
        capture_id: id.into(),
        countdown: None,
    });
    place_hud(app)?;
    arm_hud_dismiss(app);
    Ok(())
}

fn format_from_path(path: &std::path::Path) -> ImageFormat {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("png").to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::Webp,
        "tif" | "tiff" => ImageFormat::Tiff,
        "pdf" => ImageFormat::Pdf,
        "heic" => ImageFormat::Heic,
        _ => ImageFormat::Png,
    }
}

fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(err_string)
    } else {
        manager.disable().map_err(err_string)
    }
}
