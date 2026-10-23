mod input;

use image::RgbaImage;
use thiserror::Error;
use xcap::{Monitor, Window};

pub use input::{element_bounds, installed_apps, lookup_text, open_with, print_file_windows, screen_reader_active, scroll_at, share_file_windows, AppChoice};

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("{0}")]
    Message(String),
}

impl From<xcap::XCapError> for PlatformError {
    fn from(value: xcap::XCapError) -> Self {
        Self::Message(value.to_string())
    }
}

pub type PlatformResult<T> = Result<T, PlatformError>;

#[derive(Clone, Debug)]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub primary: bool,
}

#[derive(Clone, Debug)]
pub struct FrozenDisplay {
    pub info: DisplayInfo,
    pub image: RgbaImage,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub id: u32,
    pub app_name: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PermissionState {
    Granted,
    Denied { reason: String },
}

pub fn permission_status() -> PermissionState {
    if !screen_recording_granted() {
        return PermissionState::Denied {
            reason: "Screen Recording is off for Shotsmith.".into(),
        };
    }
    PermissionState::Granted
}

pub fn accessibility_granted() -> bool {
    #[cfg(target_os = "macos")]
    {
        unsafe { AXIsProcessTrusted() != 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

pub fn request_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        unsafe {
            let key = kAXTrustedCheckOptionPrompt;
            let value = kCFBooleanTrue;
            let options = CFDictionaryCreate(
                std::ptr::null(),
                &key,
                &value,
                1,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            let trusted = AXIsProcessTrustedWithOptions(options) != 0;
            if !options.is_null() {
                CFRelease(options);
            }
            trusted
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

pub fn screen_recording_granted() -> bool {
    #[cfg(target_os = "macos")]
    {
        unsafe { CGPreflightScreenCaptureAccess() != 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

pub fn request_screen_recording() -> bool {
    #[cfg(target_os = "macos")]
    {
        unsafe { CGRequestScreenCaptureAccess() != 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

pub fn open_system_settings(pane: &str) -> PlatformResult<()> {
    #[cfg(target_os = "macos")]
    {
        let urls: &[&str] = match pane {
            "keyboard" => &["x-apple.systempreferences:com.apple.preference.keyboard"],
            "accessibility" => &[
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_Accessibility",
            ],
            _ => &[
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
                "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_ScreenCapture",
            ],
        };
        let mut last = "System Settings did not open.".to_string();
        for url in urls {
            match std::process::Command::new("open").arg(url).status() {
                Ok(status) if status.success() => return Ok(()),
                Ok(status) => last = format!("System Settings did not open ({status})."),
                Err(err) => last = err.to_string(),
            }
        }
        return Err(PlatformError::Message(last));
    }
    #[cfg(target_os = "windows")]
    {
        let target = match pane {
            "keyboard" => "ms-settings:keyboard",
            _ => "ms-settings:keyboard",
        };
        match std::process::Command::new("cmd")
            .args(["/C", "start", "", target])
            .status()
        {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(PlatformError::Message(format!("Windows Settings did not open ({status})."))),
            Err(err) => Err(PlatformError::Message(err.to_string())),
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = pane;
        Err(PlatformError::Message(
            "Open the system privacy settings for screen capture.".into(),
        ))
    }
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> u8;
    fn CGRequestScreenCaptureAccess() -> u8;
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> u8;
    static kAXTrustedCheckOptionPrompt: *const std::ffi::c_void;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFBooleanTrue: *const std::ffi::c_void;
    static kCFTypeDictionaryKeyCallBacks: u8;
    static kCFTypeDictionaryValueCallBacks: u8;
    fn CFDictionaryCreate(
        allocator: *const std::ffi::c_void,
        keys: *const *const std::ffi::c_void,
        values: *const *const std::ffi::c_void,
        num_values: isize,
        key_callbacks: *const u8,
        value_callbacks: *const u8,
    ) -> *const std::ffi::c_void;
    fn CFRelease(item: *const std::ffi::c_void);
}

pub fn displays() -> PlatformResult<Vec<DisplayInfo>> {
    Ok(Monitor::all()?.iter().filter_map(display_from).collect())
}

pub fn freeze_all() -> PlatformResult<Vec<FrozenDisplay>> {
    let mut frozen = Vec::new();
    for monitor in Monitor::all()? {
        let Some(info) = display_from(&monitor) else {
            continue;
        };
        let image = monitor.capture_image()?;
        frozen.push(FrozenDisplay { info, image });
    }
    if frozen.is_empty() {
        return Err(PlatformError::Message("No displays were captured.".into()));
    }
    Ok(frozen)
}

pub fn list_windows() -> PlatformResult<Vec<WindowInfo>> {
    let mut windows = Vec::new();
    for window in Window::all()? {
        let title = window.title().unwrap_or_default();
        let app_name = window.app_name().unwrap_or_default();
        if window.is_minimized().unwrap_or(false) {
            continue;
        }
        let width = window.width().unwrap_or(0);
        let height = window.height().unwrap_or(0);
        if width < 40 || height < 40 {
            continue;
        }
        let app_lower = app_name.to_lowercase();
        let title_lower = title.to_lowercase();
        if app_lower.contains("shotsmith") || title_lower.contains("shotsmith") {
            continue;
        }
        windows.push(WindowInfo {
            id: window.id().unwrap_or(0),
            app_name,
            title,
            x: window.x().unwrap_or(0),
            y: window.y().unwrap_or(0),
            width,
            height,
        });
    }
    Ok(windows)
}

pub fn capture_window(id: u32) -> PlatformResult<RgbaImage> {
    for window in Window::all()? {
        if window.id().unwrap_or(u32::MAX) == id {
            return Ok(window.capture_image()?);
        }
    }
    Err(PlatformError::Message(format!("Window {id} is not available.")))
}

pub fn cursor_position() -> (i32, i32) {
    use device_query::{DeviceQuery, DeviceState};
    let state = DeviceState::new();
    let mouse: device_query::MouseState = state.get_mouse();
    (mouse.coords.0, mouse.coords.1)
}

pub fn reveal_path(path: &std::path::Path) -> PlatformResult<()> {
    let target = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    let result = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg("-R").arg(path).status()
    } else if cfg!(target_os = "windows") {
        // explorer.exe often exits with code 1 after a successful select.
        let selected = format!("/select,{}", path.display());
        return match std::process::Command::new("explorer").arg(selected).spawn() {
            Ok(mut child) => {
                let _ = child.wait();
                Ok(())
            }
            Err(err) => Err(PlatformError::Message(err.to_string())),
        };
    } else {
        std::process::Command::new("xdg-open").arg(target).status()
    };
    match result {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(PlatformError::Message(format!("Reveal exited {status}."))),
        Err(err) => Err(PlatformError::Message(err.to_string())),
    }
}

fn display_from(monitor: &Monitor) -> Option<DisplayInfo> {
    Some(DisplayInfo {
        id: monitor.id().ok()?,
        name: monitor
            .friendly_name()
            .or_else(|_| monitor.name())
            .unwrap_or_else(|_| "Display".into()),
        x: monitor.x().ok()?,
        y: monitor.y().ok()?,
        width: monitor.width().ok()?,
        height: monitor.height().ok()?,
        scale: monitor.scale_factor().unwrap_or(1.0).max(1.0),
        primary: monitor.is_primary().unwrap_or(false),
    })
}
