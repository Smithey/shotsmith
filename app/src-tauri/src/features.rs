use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use shotsmith_capture::PixelRect;
use shotsmith_core::{AnnotationObject, Document, ImageFormat, ObfuscateMode, ObfuscateObject};
use shotsmith_ocr::{plain_text, scan, Finding, TextLine};
use shotsmith_render::BeautifyOptions;
use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::commands::{self, err_string};
use crate::state::AppState;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeautifyRequest {
    pub scale: f32,
    pub backdrop: String,
    pub padding: f32,
    pub radius: f32,
    pub shadow: bool,
    pub frame: bool,
    pub watermark: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInState {
    pub capture_id: String,
    pub recent: Vec<shotsmith_platform::AppChoice>,
    pub apps: Vec<shotsmith_platform::AppChoice>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub uploads: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasureHit {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub(crate) fn deliver_text(app: &AppHandle, state: &State<AppState>, id: &str, image: &RgbaImage, path: &Path) -> Result<(), String> {
    let lines = shotsmith_ocr::recognize(image).map_err(err_string)?;
    finish_text(app, state, id, path, &lines)
}

pub(crate) fn copy_capture_text(app: &AppHandle, state: &State<AppState>, id: &str, image: &RgbaImage, path: &str) -> Result<(), String> {
    deliver_text(app, state, id, image, Path::new(path))
}

fn finish_text(app: &AppHandle, state: &State<AppState>, id: &str, path: &Path, lines: &[TextLine]) -> Result<(), String> {
    let text = plain_text(lines);
    if text.trim().is_empty() {
        return Err("No text was found.".into());
    }
    store_lines(path, lines)?;
    state.history.lock().map_err(err_string)?.set_ocr(id, &text).map_err(err_string)?;
    let mut clipboard = arboard::Clipboard::new().map_err(err_string)?;
    clipboard.set_text(text.clone()).map_err(err_string)?;
    let detail = text.lines().next().unwrap_or("Copied").to_string();
    commands::show_hud(app, state, "Text copied", &detail, id, &path.to_string_lossy())
}

pub(crate) fn block_secrets(state: &State<AppState>, id: &str) -> Result<(), String> {
    let warn = state.settings.lock().map_err(err_string)?.warn_on_secrets;
    if !warn {
        return Ok(());
    }
    let findings = findings_for(state, id).unwrap_or_default();
    if findings.iter().any(|item| item.kind.serious()) {
        return Err("A card number or key is still visible. Redact it in the editor, or turn the warning off in Settings.".into());
    }
    Ok(())
}

pub(crate) fn scroll_capture(
    state: &State<AppState>,
    display_id: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    first: RgbaImage,
) -> Result<RgbaImage, String> {
    let (origin_x, origin_y, scale) = {
        let displays = state.displays.lock().map_err(err_string)?;
        let display = displays.iter().find(|item| item.id == display_id).ok_or("That display is not available.")?;
        (display.x, display.y, display.scale.max(1.0))
    };
    let gx = origin_x + ((x as f32 + width as f32 / 2.0) / scale) as i32;
    let gy = origin_y + ((y as f32 + height as f32 / 2.0) / scale) as i32;
    let mut pages = vec![first];
    for _ in 0..8 {
        shotsmith_platform::scroll_at(gx, gy, -((height as i32) * 2 / 3).max(80)).map_err(err_string)?;
        std::thread::sleep(Duration::from_millis(280));
        let next = live_crop(display_id, x, y, width, height)?;
        if shotsmith_capture::pages_same(pages.last().expect("page"), &next) {
            break;
        }
        pages.push(next);
    }
    Ok(shotsmith_capture::stitch_vertical(&pages))
}

fn live_crop(display_id: u32, x: u32, y: u32, width: u32, height: u32) -> Result<RgbaImage, String> {
    let frozen = shotsmith_platform::freeze_all().map_err(err_string)?;
    let display = frozen.iter().find(|item| item.info.id == display_id).ok_or("That display is not available.")?;
    Ok(shotsmith_capture::crop(&display.image, PixelRect { x: x as i32, y: y as i32, width, height }))
}

pub(crate) fn share_capture(app: &AppHandle, state: &State<AppState>, id: &str, image: &RgbaImage) -> Result<(), String> {
    let path = write_temp(image)?;
    let window = any_window(app)?;
    run_on_ui(app, window, path, true)?;
    commands::show_hud(app, state, "Share", "Choose where it goes", id, "")
}

pub(crate) fn print_image(app: &AppHandle, image: &RgbaImage, fit: bool, rotate: bool) -> Result<(), String> {
    let prepared = prepare_print(image, fit, rotate);
    let path = write_temp(&prepared)?;
    let window = any_window(app)?;
    run_on_ui(app, window, path, false)
}

pub(crate) fn begin_open_in(app: &AppHandle, state: &State<AppState>, id: &str) -> Result<(), String> {
    *state.open_in_id.lock().map_err(err_string)? = Some(id.to_string());
    crate::windows::open_view(app, "open-in", "open-in", "Open in", 280.0, 120.0, 420.0, 520.0, true, false)
}

#[tauri::command]
pub fn editor_text(window: WebviewWindow, state: State<AppState>) -> Result<Vec<TextLine>, String> {
    let id = commands::editor_key(&window)?;
    let path = image_path(&state, &id)?;
    if let Some(lines) = read_lines(&lines_path(&path))? {
        return Ok(lines);
    }
    let image = image::open(&path).map_err(err_string)?.to_rgba8();
    let lines = shotsmith_ocr::recognize(&image).map_err(err_string)?;
    store_lines(Path::new(&path), &lines)?;
    let text = plain_text(&lines);
    state.history.lock().map_err(err_string)?.set_ocr(&id, &text).map_err(err_string)?;
    Ok(lines)
}

#[tauri::command]
pub fn editor_secrets(window: WebviewWindow, state: State<AppState>) -> Result<Vec<Finding>, String> {
    let id = commands::editor_key(&window)?;
    findings_for(&state, &id)
}

#[tauri::command]
pub fn editor_dismiss_secret(window: WebviewWindow, state: State<AppState>, finding_id: String) -> Result<Vec<Finding>, String> {
    let id = commands::editor_key(&window)?;
    let path = image_path(&state, &id)?;
    let mut dismissed = read_strings(&dismissed_path(&path))?.unwrap_or_default();
    if !dismissed.iter().any(|item| item == &finding_id) {
        dismissed.push(finding_id);
    }
    write_json(&dismissed_path(&path), &dismissed)?;
    findings_for(&state, &id)
}

#[tauri::command]
pub fn editor_redact(window: WebviewWindow, app: AppHandle, state: State<AppState>) -> Result<serde_json::Value, String> {
    let id = commands::editor_key(&window)?;
    let findings = findings_for(&state, &id)?;
    let response = {
        let mut editors = state.editors.lock().map_err(err_string)?;
        let session = editors.get_mut(&id).ok_or("No capture is open.")?;
        let mut document = session.stack.current().clone();
        for finding in &findings {
            document.objects.push(AnnotationObject::Obfuscate(ObfuscateObject {
                id: uuid::Uuid::new_v4().to_string(),
                x: finding.x,
                y: finding.y,
                w: finding.w.max(8.0),
                h: finding.h.max(8.0),
                mode: ObfuscateMode::Solid,
                block_size: 8,
                color: "#1D1D1F".into(),
            }));
        }
        let changed = commands::record_edit(session, document, None);
        let response = commands::with_pixels(session, false);
        let image_path = session.image_path.clone();
        drop(editors);
        if changed {
            commands::publish_edits(&app, &state, &id, &image_path)?;
        }
        response
    };
    Ok(response)
}

#[tauri::command]
pub fn editor_share(window: WebviewWindow, app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let id = commands::editor_key(&window)?;
    block_secrets(&state, &id)?;
    let image = {
        let mut editors = state.editors.lock().map_err(err_string)?;
        let session = editors.get_mut(&id).ok_or("No capture is open.")?;
        commands::flattened(session)?
    };
    let path = write_temp(&image)?;
    run_on_ui(&app, window, path, true)?;
    commands::show_hud(&app, &state, "Share", "Choose where it goes", &id, "")
}

#[tauri::command]
pub fn editor_print(window: WebviewWindow, app: AppHandle, state: State<AppState>, fit: bool, rotate: bool) -> Result<(), String> {
    let id = commands::editor_key(&window)?;
    block_secrets(&state, &id)?;
    let image = {
        let mut editors = state.editors.lock().map_err(err_string)?;
        let session = editors.get_mut(&id).ok_or("No capture is open.")?;
        commands::flattened(session)?
    };
    let prepared = prepare_print(&image, fit, rotate);
    let path = write_temp(&prepared)?;
    run_on_ui(&app, window, path, false)
}

#[tauri::command]
pub fn editor_beautify(window: WebviewWindow, app: AppHandle, state: State<AppState>, options: BeautifyRequest) -> Result<serde_json::Value, String> {
    let id = commands::editor_key(&window)?;
    let response = {
        let mut editors = state.editors.lock().map_err(err_string)?;
        let session = editors.get_mut(&id).ok_or("No capture is open.")?;
        let before = std::fs::read(&session.image_path).map_err(err_string)?;
        let flat = commands::flattened(session)?;
        let pretty = shotsmith_render::beautify(
            &flat,
            &BeautifyOptions {
                scale: options.scale,
                backdrop: options.backdrop,
                padding: options.padding,
                radius: options.radius,
                shadow: options.shadow,
                frame: options.frame,
                watermark: options.watermark,
            },
        );
        let png = shotsmith_output::encode(&pretty, ImageFormat::Png, 1.0).map_err(err_string)?;
        shotsmith_output::write_atomic(Path::new(&session.image_path), &png).map_err(err_string)?;
        session.image = Some(pretty.clone());
        commands::record_edit(session, Document::new(pretty.width(), pretty.height()), Some(before));
        let response = commands::with_pixels(session, true);
        let image_path = session.image_path.clone();
        drop(editors);
        commands::publish_edits(&app, &state, &id, &image_path)?;
        response
    };
    Ok(response)
}

#[tauri::command]
pub fn apps_list() -> Vec<shotsmith_platform::AppChoice> {
    shotsmith_platform::installed_apps()
}

#[tauri::command]
pub fn open_in_state(state: State<AppState>) -> Result<OpenInState, String> {
    let capture_id = state.open_in_id.lock().map_err(err_string)?.clone().unwrap_or_default();
    let recent_ids = state.settings.lock().map_err(err_string)?.recent_apps.clone();
    let apps = shotsmith_platform::installed_apps();
    let recent = recent_ids
        .iter()
        .filter_map(|id| apps.iter().find(|app| &app.id == id).cloned())
        .collect();
    Ok(OpenInState { capture_id, recent, apps })
}

#[tauri::command]
pub fn open_in_choose(app: AppHandle, state: State<AppState>, app_id: String, capture_id: Option<String>) -> Result<(), String> {
    let capture_id = capture_id.or(state.open_in_id.lock().map_err(err_string)?.clone()).ok_or("No capture is waiting.")?;
    let record = state.history.lock().map_err(err_string)?.get(&capture_id).map_err(err_string)?;
    let record = record.ok_or("That capture is no longer in History.")?;
    shotsmith_platform::open_with(&app_id, Path::new(&record.original_path)).map_err(err_string)?;
    {
        let mut settings = state.settings.lock().map_err(err_string)?;
        settings.recent_apps.retain(|item| item != &app_id);
        settings.recent_apps.insert(0, app_id);
        settings.recent_apps.truncate(5);
        let snapshot = settings.clone();
        drop(settings);
        crate::state::save_settings(&state.root, &snapshot)?;
    }
    if let Some(window) = app.get_webview_window("open-in") {
        let _ = window.close();
    }
    Ok(())
}

#[tauri::command]
pub fn measure_at(x: f32, y: f32) -> Result<MeasureHit, String> {
    let (left, top, w, h) = shotsmith_platform::element_bounds(x, y).map_err(err_string)?;
    Ok(MeasureHit { x: left, y: top, w, h })
}

#[tauri::command]
pub fn lookup_text(text: String) -> Result<(), String> {
    shotsmith_platform::lookup_text(&text).map_err(err_string)
}

#[tauri::command]
pub fn plugins_list(state: State<AppState>) -> Result<Vec<PluginInfo>, String> {
    let dir = state.root.join("Plugins");
    let Ok(read) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut plugins = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let Some(id) = value.get("id").and_then(|item| item.as_str()) else {
            continue;
        };
        plugins.push(PluginInfo {
            id: id.to_string(),
            name: value.get("name").and_then(|item| item.as_str()).unwrap_or(id).to_string(),
            uploads: value.get("uploads").and_then(|item| item.as_str()).unwrap_or("This plugin does not say what it uploads.").to_string(),
        });
    }
    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(plugins)
}

#[tauri::command]
pub fn editor_plugin(window: WebviewWindow, state: State<AppState>, plugin_id: String) -> Result<(), String> {
    if !state.settings.lock().map_err(err_string)?.plugins_enabled {
        return Err("Plugins are off. Turn them on in Settings.".into());
    }
    let id = commands::editor_key(&window)?;
    let path = image_path(&state, &id)?;
    let manifest = plugin_command(&state.root, &plugin_id)?;
    std::process::Command::new(&manifest).arg(&path).spawn().map_err(err_string)?;
    Ok(())
}

#[tauri::command]
pub fn updates_check(state: State<AppState>) -> Result<String, String> {
    if !state.settings.lock().map_err(err_string)?.check_updates {
        return Err("Update checks are off.".into());
    }
    let response = ureq::get("https://api.github.com/repos/Smithey/shotsmith/releases/latest")
        .set("User-Agent", "Shotsmith")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(err_string)?
        .into_string()
        .map_err(err_string)?;
    let body: serde_json::Value = serde_json::from_str(&response).map_err(err_string)?;
    let tag = body.get("tag_name").and_then(|value| value.as_str()).unwrap_or("unknown");
    let current = env!("CARGO_PKG_VERSION");
    let latest = tag.trim_start_matches('v');
    if latest == current || latest.starts_with(&format!("{current}-")) {
        Ok(format!("You're on {current}."))
    } else {
        Ok(format!("You're on {current}. Latest release is {tag}."))
    }
}

pub fn spawn_watch(app: AppHandle) {
    std::thread::spawn(move || {
        let started = SystemTime::now();
        let mut seen: Vec<(PathBuf, u64, Instant, bool)> = Vec::new();
        loop {
            let folders = app.state::<AppState>().settings.lock().map(|settings| settings.watch_folders.clone()).unwrap_or_default();
            for folder in folders {
                let Ok(read) = std::fs::read_dir(&folder) else {
                    continue;
                };
                for entry in read.flatten() {
                    let path = entry.path();
                    if !is_image(&path) {
                        continue;
                    }
                    let Ok(meta) = entry.metadata() else {
                        continue;
                    };
                    let size = meta.len();
                    let older = meta.modified().ok().and_then(|time| time.duration_since(started).err()).is_some()
                        && meta.modified().ok().is_some_and(|time| time < started);
                    if let Some(slot) = seen.iter_mut().find(|(item, _, _, _)| item == &path) {
                        if slot.3 {
                            continue;
                        }
                        if slot.1 != size {
                            slot.1 = size;
                            slot.2 = Instant::now();
                        } else if slot.2.elapsed() >= Duration::from_secs(1) {
                            let state = app.state::<AppState>();
                            if commands::import_external(&app, &state, &path).is_ok() {
                                slot.3 = true;
                            }
                        }
                    } else {
                        seen.push((path, size, Instant::now(), older));
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

fn plugin_command(root: &Path, id: &str) -> Result<String, String> {
    let dir = root.join("Plugins");
    let read = std::fs::read_dir(&dir).map_err(|_| "No plugins are installed.".to_string())?;
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if value.get("id").and_then(|item| item.as_str()) == Some(id) {
            return value.get("command").and_then(|item| item.as_str()).map(|item| item.to_string()).ok_or_else(|| "That plugin has no command.".into());
        }
    }
    Err("That plugin is not installed.".into())
}

fn findings_for(state: &State<AppState>, id: &str) -> Result<Vec<Finding>, String> {
    let path = image_path(state, id)?;
    let lines = if let Some(lines) = read_lines(&lines_path(&path))? {
        lines
    } else {
        let image = image::open(&path).map_err(err_string)?.to_rgba8();
        let lines = shotsmith_ocr::recognize(&image).map_err(err_string)?;
        store_lines(Path::new(&path), &lines)?;
        let text = plain_text(&lines);
        state.history.lock().map_err(err_string)?.set_ocr(id, &text).map_err(err_string)?;
        lines
    };
    let dismissed = read_strings(&dismissed_path(&path))?.unwrap_or_default();
    Ok(scan(&lines, &dismissed))
}

fn image_path(state: &State<AppState>, id: &str) -> Result<String, String> {
    if let Ok(editors) = state.editors.lock() {
        if let Some(session) = editors.get(id) {
            return Ok(session.image_path.clone());
        }
    }
    let record = state.history.lock().map_err(err_string)?.get(id).map_err(err_string)?;
    record.map(|item| item.original_path).ok_or_else(|| "That capture is no longer in History.".into())
}

fn lines_path(original: &str) -> PathBuf {
    PathBuf::from(original).with_file_name("ocr.json")
}

fn dismissed_path(original: &str) -> PathBuf {
    PathBuf::from(original).with_file_name("dismissed.json")
}

fn store_lines(original: &Path, lines: &[TextLine]) -> Result<(), String> {
    write_json(&original.with_file_name("ocr.json"), lines)
}

fn read_lines(path: &Path) -> Result<Option<Vec<TextLine>>, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    serde_json::from_str(&text).map(Some).map_err(err_string)
}

fn read_strings(path: &Path) -> Result<Option<Vec<String>>, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    serde_json::from_str(&text).map(Some).map_err(err_string)
}

fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_string(value).map_err(err_string)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err_string)?;
    }
    std::fs::write(path, json).map_err(err_string)
}

fn write_temp(image: &RgbaImage) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!("shotsmith-{}.png", uuid::Uuid::new_v4()));
    let bytes = shotsmith_output::encode(image, ImageFormat::Png, 1.0).map_err(err_string)?;
    std::fs::write(&path, bytes).map_err(err_string)?;
    Ok(path)
}

fn prepare_print(image: &RgbaImage, fit: bool, rotate: bool) -> RgbaImage {
    let image = if rotate { image::imageops::rotate90(image) } else { image.clone() };
    if !fit {
        return image;
    }
    let max_w = 2550.0;
    let max_h = 3300.0;
    let scale = (max_w / image.width() as f32).min(max_h / image.height() as f32).min(1.0);
    if scale >= 0.999 {
        return image;
    }
    image::imageops::resize(
        &image,
        ((image.width() as f32) * scale).round().max(1.0) as u32,
        ((image.height() as f32) * scale).round().max(1.0) as u32,
        image::imageops::FilterType::Triangle,
    )
}

fn any_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.webview_windows().into_values().next().ok_or_else(|| "No window is open.".into())
}

fn run_on_ui(app: &AppHandle, window: WebviewWindow, path: PathBuf, share: bool) -> Result<(), String> {
    let call = move || {
        if share {
            crate::destinations::share_path(&window, &path)
        } else {
            crate::destinations::print_path(&window, &path)
        }
    };
    if commands::on_main_thread() {
        return call();
    }
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(call());
    })
    .map_err(err_string)?;
    rx.recv().map_err(|_| "The window did not answer.".to_string())?
}

fn is_image(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase()).as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "tif" | "tiff")
    )
}

pub fn dispatch_args(app: &AppHandle, args: &[String]) -> bool {
    let mut handled = false;
    for arg in args {
        let Some(rest) = arg.trim().strip_prefix("shotsmith://") else {
            continue;
        };
        handled = true;
        let path = rest.split(['?', '#']).next().unwrap_or("").trim_matches('/');
        let mode = match path {
            "capture/window" | "window" => "window",
            "capture/screen" | "screen" => "screen",
            "capture/text" | "text" => "text",
            "capture/scroll" | "scroll" => "scroll",
            "capture/measure" | "measure" => "measure",
            "capture" | "capture/region" | "region" => "region",
            _ => continue,
        };
        let _ = commands::capture_begin(app.clone(), app.state(), mode.into());
    }
    handled
}
