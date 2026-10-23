use serde::{Deserialize, Serialize};
use shotsmith_core::{builtin_presets, Document, Preset, UndoStack};
use shotsmith_history::HistoryDb;
use shotsmith_platform::FrozenDisplay;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutMap {
    pub region: String,
    pub repeat_last: String,
    pub window: String,
    pub full_screen: String,
    pub history: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub launch_at_login: bool,
    pub show_dock_icon: bool,
    pub sound: bool,
    pub hud_enabled: bool,
    pub appearance: String,
    pub loupe: bool,
    pub overlay_toolbar: bool,
    pub include_cursor: bool,
    pub after_capture: String,
    pub default_preset_id: String,
    pub shortcuts: ShortcutMap,
    pub donut_slots: Vec<String>,
    pub donut_count: u8,
    pub history_limit: u32,
    pub history_days: u32,
    pub keep_history: bool,
    pub pins_persist: bool,
    pub onboarding_done: bool,
    pub timed_seconds: u32,
    pub timed_mode: String,
    pub filename_counter: u32,
    #[serde(default = "default_true")]
    pub warn_on_secrets: bool,
    #[serde(default)]
    pub watch_folders: Vec<String>,
    #[serde(default)]
    pub recent_apps: Vec<String>,
    #[serde(default)]
    pub check_updates: bool,
    #[serde(default)]
    pub plugins_enabled: bool,
    #[serde(default = "default_locale")]
    pub locale: String,
}

fn default_true() -> bool {
    true
}

fn default_locale() -> String {
    "en".into()
}

impl Default for Settings {
    fn default() -> Self {
        let mac = cfg!(target_os = "macos");
        Self {
            launch_at_login: false,
            show_dock_icon: false,
            sound: false,
            hud_enabled: true,
            appearance: "system".into(),
            loupe: true,
            overlay_toolbar: true,
            include_cursor: false,
            after_capture: "donut".into(),
            default_preset_id: "builtin-docs-png".into(),
            shortcuts: if mac {
                ShortcutMap {
                    region: "super+shift+digit2".into(),
                    repeat_last: "super+shift+digit1".into(),
                    window: "alt+super+shift+digit2".into(),
                    full_screen: "control+super+shift+digit2".into(),
                    history: "super+keyy".into(),
                }
            } else {
                ShortcutMap {
                    region: "control+shift+digit2".into(),
                    repeat_last: "control+shift+digit1".into(),
                    window: "alt+control+shift+digit2".into(),
                    full_screen: "super+control+shift+digit2".into(),
                    history: "control+keyh".into(),
                }
            },
            donut_slots: vec![
                "copy".into(),
                "save".into(),
                "annotate".into(),
                "pin".into(),
                "share".into(),
                "copy-text".into(),
                "preset:builtin-docs-png".into(),
                "open-in".into(),
            ],
            donut_count: 8,
            history_limit: 50,
            history_days: 30,
            keep_history: true,
            pins_persist: true,
            onboarding_done: false,
            timed_seconds: 5,
            timed_mode: "region".into(),
            filename_counter: 1,
            warn_on_secrets: true,
            watch_folders: Vec::new(),
            recent_apps: Vec::new(),
            check_updates: false,
            plugins_enabled: false,
            locale: "en".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayPayload {
    pub id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub image_path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DonutPayload {
    pub capture_id: String,
    pub width: u32,
    pub height: u32,
    pub image_path: String,
    pub preview: String,
    pub slots: Vec<DonutSlotPayload>,
    pub cursor_x: i32,
    pub cursor_y: i32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DonutSlotPayload {
    pub id: String,
    pub label: String,
    pub key: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HudPayload {
    pub title: String,
    pub detail: String,
    pub image_path: String,
    pub capture_id: String,
    pub countdown: Option<u32>,
}

pub struct EditorSession {
    pub capture_id: String,
    pub stack: UndoStack,
    pub image_path: String,
    pub dirty: bool,
    /// Pixel snapshots lined up with the undo stack. `None` means that step did not change the image.
    pub pixels_past: Vec<Option<Vec<u8>>>,
    pub pixels_future: Vec<Option<Vec<u8>>>,
    /// Decoded capture. Cleared when a crop replaces the file.
    pub image: Option<image::RgbaImage>,
}

#[derive(Clone)]
pub struct LastRegion {
    pub display_id: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub struct AppState {
    pub root: PathBuf,
    pub settings: Mutex<Settings>,
    pub presets: Mutex<Vec<Preset>>,
    pub history: Mutex<HistoryDb>,
    pub frozen: Mutex<Option<Vec<FrozenDisplay>>>,
    pub displays: Mutex<Vec<DisplayPayload>>,
    pub last_region: Mutex<Option<LastRegion>>,
    pub editors: Mutex<HashMap<String, EditorSession>>,
    pub donut: Mutex<Option<DonutPayload>>,
    pub hud: Mutex<Option<HudPayload>>,
    pub capture_mode: Mutex<String>,
    pub open_in_id: Mutex<Option<String>>,
}

pub fn app_root() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("Shotsmith")
}

pub fn load_settings(root: &Path) -> Settings {
    let path = root.join("settings.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_settings(root: &Path, settings: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|err| err.to_string())?;
    let json = serde_json::to_string_pretty(settings).map_err(|err| err.to_string())?;
    std::fs::write(root.join("settings.json"), json).map_err(|err| err.to_string())
}

pub fn load_presets(root: &Path) -> Vec<Preset> {
    let folder = default_folder();
    let path = root.join("presets.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<Preset>>(&text).ok())
        .map(|mut list| {
            for preset in &mut list {
                if preset.id == "builtin-docs-png" && preset.name == "Docs PNG" {
                    preset.name = "Save PNG".into();
                }
            }
            list
        })
        .unwrap_or_else(|| builtin_presets(&folder))
}

pub fn save_presets(root: &Path, presets: &[Preset]) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|err| err.to_string())?;
    let json = serde_json::to_string_pretty(presets).map_err(|err| err.to_string())?;
    std::fs::write(root.join("presets.json"), json).map_err(|err| err.to_string())
}

pub fn default_folder() -> String {
    dirs::picture_dir()
        .or_else(dirs::document_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Shotsmith")
        .to_string_lossy()
        .to_string()
}

pub fn read_document(path: &Path) -> Result<(Document, Vec<u8>), String> {
    if path.extension().and_then(|ext| ext.to_str()) == Some("shotsmith") {
        shotsmith_output::read_shotsmith(path).map_err(|err| err.to_string())
    } else {
        Err("Not a .shotsmith document.".into())
    }
}
