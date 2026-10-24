mod input;
#[cfg(target_os = "macos")]
mod macos_capture;

use image::RgbaImage;
use thiserror::Error;
#[cfg(not(target_os = "macos"))]
use xcap::Monitor;
use xcap::Window;

pub use input::{
    app_pid_at, element_bounds, installed_apps, lookup_text, open_with, print_file_windows,
    screen_reader_active, scroll_at, share_file_windows, AppChoice,
};

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
            Ok(status) => Err(PlatformError::Message(format!(
                "Windows Settings did not open ({status})."
            ))),
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
    #[cfg(target_os = "macos")]
    {
        return macos_capture::display_list();
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(Monitor::all()?.iter().filter_map(display_from).collect())
    }
}

pub fn freeze_all() -> PlatformResult<Vec<FrozenDisplay>> {
    #[cfg(target_os = "macos")]
    {
        return macos_capture::freeze_all();
    }
    #[cfg(not(target_os = "macos"))]
    {
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
}

pub fn capture_region(
    display_id: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> PlatformResult<RgbaImage> {
    #[cfg(target_os = "macos")]
    {
        return macos_capture::capture_region(display_id, x, y, width, height);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let frozen = freeze_all()?;
        let display = frozen
            .iter()
            .find(|item| item.info.id == display_id)
            .ok_or_else(|| PlatformError::Message("That display is not available.".into()))?;
        Ok(crop_rgba(&display.image, x, y, width, height))
    }
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
    #[cfg(target_os = "macos")]
    {
        return macos_capture::capture_window(id);
    }
    #[cfg(not(target_os = "macos"))]
    {
        for window in Window::all()? {
            if window.id().unwrap_or(u32::MAX) == id {
                return Ok(window.capture_image()?);
            }
        }
        Err(PlatformError::Message(format!(
            "Window {id} is not available."
        )))
    }
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
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .status()
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

#[cfg(not(target_os = "macos"))]
fn crop_rgba(image: &RgbaImage, x: u32, y: u32, width: u32, height: u32) -> RgbaImage {
    let x = x.min(image.width().saturating_sub(1));
    let y = y.min(image.height().saturating_sub(1));
    let width = width.max(1).min(image.width() - x);
    let height = height.max(1).min(image.height() - y);
    image::imageops::crop_imm(image, x, y, width, height).to_image()
}

#[cfg(any(test, target_os = "macos"))]
pub(crate) fn region_points(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    pixel_w: u32,
    pixel_h: u32,
    point_w: f64,
    point_h: f64,
) -> (f64, f64, f64, f64) {
    let sx = if pixel_w == 0 {
        1.0
    } else {
        point_w / pixel_w as f64
    };
    let sy = if pixel_h == 0 {
        1.0
    } else {
        point_h / pixel_h as f64
    };
    (
        x as f64 * sx,
        y as f64 * sy,
        (width as f64 * sx).max(1.0),
        (height as f64 * sy).max(1.0),
    )
}

#[cfg(any(test, target_os = "macos"))]
pub(crate) fn bgra_to_rgba(
    width: usize,
    height: usize,
    bytes_per_row: usize,
    data: &[u8],
) -> PlatformResult<RgbaImage> {
    if width == 0 || height == 0 || bytes_per_row < width.saturating_mul(4) {
        return Err(PlatformError::Message(
            "The screen capture was empty.".into(),
        ));
    }
    let mut buffer = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        let start = row * bytes_per_row;
        let end = start + width * 4;
        if end > data.len() {
            return Err(PlatformError::Message(
                "The screen capture was empty.".into(),
            ));
        }
        buffer.extend_from_slice(&data[start..end]);
    }
    for pixel in buffer.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    RgbaImage::from_raw(width as u32, height as u32, buffer)
        .ok_or_else(|| PlatformError::Message("The screen capture was empty.".into()))
}

#[cfg(not(target_os = "macos"))]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_rows_become_rgba() {
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let image = bgra_to_rgba(2, 1, 8, &data).unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [3, 2, 1, 4]);
        assert_eq!(image.get_pixel(1, 0).0, [7, 6, 5, 8]);
    }

    #[test]
    fn bgra_skips_row_padding() {
        let data = [1, 2, 3, 4, 9, 9, 5, 6, 7, 8, 9, 9];
        let image = bgra_to_rgba(1, 2, 6, &data).unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [3, 2, 1, 4]);
        assert_eq!(image.get_pixel(0, 1).0, [7, 6, 5, 8]);
    }

    #[test]
    fn bgra_rejects_a_short_buffer() {
        assert!(bgra_to_rgba(2, 1, 8, &[1, 2, 3, 4]).is_err());
    }

    #[test]
    fn region_points_follow_the_display_scale() {
        let (x, y, w, h) = region_points(100, 40, 10, 20, 2000, 1000, 1000.0, 500.0);
        assert_eq!((x, y, w, h), (50.0, 20.0, 5.0, 10.0));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn capture_returns_without_blocking_forever() {
        let started = std::time::Instant::now();
        let result = std::thread::spawn(freeze_all)
            .join()
            .expect("capture thread");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "capture did not return"
        );
        if let Ok(frames) = &result {
            assert!(!frames.is_empty());
            assert!(frames[0].image.width() > 1);
            assert!(frames[0].image.height() > 1);
        }
    }
}
