use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::{PlatformError, PlatformResult};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppChoice {
    pub id: String,
    pub name: String,
}

/// Process id of the frontmost non-Shotsmith window under a point, in the
/// same top-left display coordinates `scroll_at` uses.
pub fn app_pid_at(x: i32, y: i32) -> Option<i32> {
    let windows = xcap::Window::all().ok()?;
    for window in windows {
        let name = window.app_name().unwrap_or_default().to_lowercase();
        if name.contains("shotsmith") || name.is_empty() {
            continue;
        }
        let (Ok(wx), Ok(wy), Ok(ww), Ok(wh)) = (window.x(), window.y(), window.width(), window.height()) else {
            continue;
        };
        if ww < 40 || wh < 40 {
            continue;
        }
        if x >= wx && y >= wy && x < wx + ww as i32 && y < wy + wh as i32 {
            return window.pid().ok().map(|pid| pid as i32);
        }
    }
    None
}

pub fn scroll_at(x: i32, y: i32, pixels: i32) -> PlatformResult<()> {
    #[cfg(target_os = "macos")]
    {
        unsafe {
            let point = CGPoint { x: x as f64, y: y as f64 };
            CGWarpMouseCursorPosition(point);
            // A warp does not update which view is under the pointer, so the
            // scroll would land on whatever was hovered before the capture.
            let moved = CGEventCreateMouseEvent(std::ptr::null_mut(), 5, point, 0);
            if !moved.is_null() {
                CGEventPost(0, moved);
                CFRelease(moved as *const std::ffi::c_void);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
            // Pixel scrolls are dropped unless they look like a trackpad
            // gesture: continuous, with a began phase and then an ended phase.
            post_scroll(point, pixels, 1)?;
            post_scroll(point, 0, 4)?;
        }
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        unsafe {
            SetCursorPos(x, y);
            // mouse_event wheel data is in WHEEL_DELTA (120) units.
            let notches = (pixels / 40).clamp(-8, 8);
            let delta = notches.saturating_mul(120);
            mouse_event(0x0800, 0, 0, delta as u32, 0);
        }
        return Ok(());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (x, y, pixels);
        Err(PlatformError::Message("Scrolling is not available on this system.".into()))
    }
}

pub fn element_bounds(x: f32, y: f32) -> PlatformResult<(f32, f32, f32, f32)> {
    #[cfg(target_os = "macos")]
    {
        return element_bounds_mac(x, y);
    }
    #[cfg(target_os = "windows")]
    {
        return element_bounds_windows(x, y);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (x, y);
        Err(PlatformError::Message("Measuring elements is not available on this system.".into()))
    }
}

pub fn installed_apps() -> Vec<AppChoice> {
    let mut apps = Vec::new();
    #[cfg(target_os = "macos")]
    {
        collect_apps(Path::new("/Applications"), &mut apps);
        if let Some(home) = dirs_home() {
            collect_apps(&home.join("Applications"), &mut apps);
        }
    }
    #[cfg(target_os = "windows")]
    {
        collect_app_paths(&mut apps);
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.id == b.id);
    apps.truncate(80);
    apps
}

pub fn open_with(app_id: &str, file: &Path) -> PlatformResult<()> {
    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open").arg("-a").arg(app_id).arg(file).status();
        return match status {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(PlatformError::Message(format!("{app_id} did not open ({status})."))),
            Err(err) => Err(PlatformError::Message(err.to_string())),
        };
    }
    #[cfg(target_os = "windows")]
    {
        let status = Command::new("cmd").args(["/C", "start", "", app_id, &file.display().to_string()]).status();
        return match status {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(PlatformError::Message(format!("{app_id} did not open ({status})."))),
            Err(err) => Err(PlatformError::Message(err.to_string())),
        };
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (app_id, file);
        Err(PlatformError::Message("Open in is not available on this system.".into()))
    }
}

pub fn lookup_text(text: &str) -> PlatformResult<()> {
    #[cfg(target_os = "macos")]
    {
        let word = text.split_whitespace().next().unwrap_or(text).trim();
        if word.is_empty() {
            return Err(PlatformError::Message("Select a word first.".into()));
        }
        let url = format!("dict://{}", url_escape(word));
        let status = Command::new("open").arg(url).status();
        return match status {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(PlatformError::Message(format!("Look Up did not open ({status})."))),
            Err(err) => Err(PlatformError::Message(err.to_string())),
        };
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Err(PlatformError::Message("Look Up is only on Mac.".into()))
    }
}

pub fn screen_reader_active() -> bool {
    #[cfg(target_os = "macos")]
    {
        Command::new("defaults")
            .args(["read", "com.apple.universalaccess", "voiceOverOnOffKey"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|text| text.trim() == "1")
            .unwrap_or(false)
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("reg")
            .args(["query", r"HKCU\Software\Microsoft\Narrator\NoRoam", "/v", "RunningState"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|text| text.contains("0x1") || text.trim().ends_with("1"))
            .unwrap_or(false)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

pub fn share_file_windows(path: &Path) -> PlatformResult<()> {
    let script = format!(
        "$path = '{}'; $shell = New-Object -ComObject Shell.Application; $folder = $shell.NameSpace((Split-Path -LiteralPath $path)); $item = $folder.ParseName((Split-Path -Leaf -LiteralPath $path)); if (-not $item) {{ exit 1 }}; $item.InvokeVerb('share')",
        path.display().to_string().replace('\'', "''")
    );
    match Command::new("powershell").args(["-NoProfile", "-STA", "-Command", &script]).status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(PlatformError::Message(format!("Windows share did not open ({status})."))),
        Err(err) => Err(PlatformError::Message(err.to_string())),
    }
}

pub fn print_file_windows(path: &Path) -> PlatformResult<()> {
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $path = '{}'; $img = [System.Drawing.Image]::FromFile($path); $dlg = New-Object System.Windows.Forms.PrintDialog; if ($dlg.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) {{ exit 0 }}; $doc = New-Object System.Drawing.Printing.PrintDocument; $doc.PrinterSettings = $dlg.PrinterSettings; $doc.Add_PrintPage({{ param($sender, $e) $e.Graphics.DrawImage($img, $e.MarginBounds) }}); $doc.Print()",
        path.display().to_string().replace('\'', "''")
    );
    match Command::new("powershell").args(["-NoProfile", "-STA", "-Command", &script]).status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(PlatformError::Message(format!("Windows print did not open ({status})."))),
        Err(err) => Err(PlatformError::Message(err.to_string())),
    }
}

fn url_escape(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(target_os = "macos")]
fn collect_apps(dir: &Path, apps: &mut Vec<AppChoice>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("app") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        apps.push(AppChoice { id: path.to_string_lossy().to_string(), name: name.to_string() });
    }
}

#[cfg(target_os = "windows")]
fn collect_app_paths(apps: &mut Vec<AppChoice>) {
    let output = Command::new("reg")
        .args(["query", r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths"])
        .output();
    let Ok(output) = output else {
        return;
    };
    let Ok(text) = String::from_utf8(output.stdout) else {
        return;
    };
    for line in text.lines() {
        let line = line.trim();
        let Some(name) = line.rsplit('\\').next() else {
            continue;
        };
        if !name.to_ascii_lowercase().ends_with(".exe") {
            continue;
        }
        apps.push(AppChoice { id: name.to_string(), name: name.trim_end_matches(".exe").to_string() });
    }
}

#[cfg(target_os = "macos")]
fn element_bounds_mac(x: f32, y: f32) -> PlatformResult<(f32, f32, f32, f32)> {
    unsafe {
        let system = AXUIElementCreateSystemWide();
        if system.is_null() {
            return Err(PlatformError::Message("Accessibility is not available.".into()));
        }
        let mut element: *const std::ffi::c_void = std::ptr::null();
        let found = AXUIElementCopyElementAtPosition(system, x, y, &mut element);
        CFRelease(system);
        if found != 0 || element.is_null() {
            return Err(PlatformError::Message("No element under the pointer.".into()));
        }
        let attr = CFStringCreateWithCString(std::ptr::null(), c"AXFrame".as_ptr(), 0x08000100);
        let mut value: *const std::ffi::c_void = std::ptr::null();
        let copied = AXUIElementCopyAttributeValue(element, attr, &mut value);
        CFRelease(element);
        if !attr.is_null() {
            CFRelease(attr);
        }
        if copied != 0 || value.is_null() {
            return Err(PlatformError::Message("That element has no frame.".into()));
        }
        let mut rect = CGRect { origin: CGPoint { x: 0.0, y: 0.0 }, size: CGSize { width: 0.0, height: 0.0 } };
        let ok = AXValueGetValue(value, 3, &mut rect as *mut CGRect as *mut std::ffi::c_void);
        CFRelease(value);
        if !ok || rect.size.width < 1.0 || rect.size.height < 1.0 {
            return Err(PlatformError::Message("That element has no frame.".into()));
        }
        Ok((rect.origin.x as f32, rect.origin.y as f32, rect.size.width as f32, rect.size.height as f32))
    }
}

#[cfg(target_os = "windows")]
fn element_bounds_windows(x: f32, y: f32) -> PlatformResult<(f32, f32, f32, f32)> {
    let script = format!(
        "Add-Type -AssemblyName UIAutomationClient; $el = [System.Windows.Automation.AutomationElement]::FromPoint((New-Object System.Windows.Point({x},{y}))); if (-not $el) {{ exit 1 }}; $r = $el.Current.BoundingRectangle; Write-Output ($r.X.ToString() + ' ' + $r.Y.ToString() + ' ' + $r.Width.ToString() + ' ' + $r.Height.ToString())"
    );
    let output = Command::new("powershell").args(["-NoProfile", "-Command", &script]).output().map_err(|err| PlatformError::Message(err.to_string()))?;
    if !output.status.success() {
        return Err(PlatformError::Message("No element under the pointer.".into()));
    }
    let text = String::from_utf8(output.stdout).map_err(|err| PlatformError::Message(err.to_string()))?;
    let nums: Vec<f32> = text.split_whitespace().filter_map(|part| part.parse().ok()).collect();
    if nums.len() != 4 || nums[2] < 1.0 || nums[3] < 1.0 {
        return Err(PlatformError::Message("That element has no frame.".into()));
    }
    Ok((nums[0], nums[1], nums[2], nums[3]))
}

#[cfg(target_os = "macos")]
fn post_scroll(point: CGPoint, delta: i32, phase: i64) -> PlatformResult<()> {
    unsafe {
        let event = CGEventCreateScrollWheelEvent(std::ptr::null_mut(), 0, 1, delta);
        if event.is_null() {
            return Err(PlatformError::Message("Could not scroll.".into()));
        }
        CGEventSetIntegerValueField(event, 88, 1);
        CGEventSetIntegerValueField(event, 96, delta as i64);
        CGEventSetIntegerValueField(event, 93, (delta as i64) << 16);
        CGEventSetIntegerValueField(event, 99, phase);
        CGEventSetIntegerValueField(event, 123, 0);
        CGEventSetLocation(event, point);
        CGEventPost(0, event);
        CFRelease(event as *const std::ffi::c_void);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGSize {
    width: f64,
    height: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWarpMouseCursorPosition(point: CGPoint) -> i32;
    fn CGEventCreateMouseEvent(source: *mut std::ffi::c_void, kind: u32, point: CGPoint, button: u32) -> *mut std::ffi::c_void;
    fn CGEventCreateScrollWheelEvent(source: *mut std::ffi::c_void, units: u32, wheel_count: u32, wheel1: i32) -> *mut std::ffi::c_void;
    fn CGEventSetIntegerValueField(event: *mut std::ffi::c_void, field: u32, value: i64);
    fn CGEventSetLocation(event: *mut std::ffi::c_void, point: CGPoint);
    fn CGEventPost(tap: u32, event: *mut std::ffi::c_void);
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> *const std::ffi::c_void;
    fn AXUIElementCopyElementAtPosition(app: *const std::ffi::c_void, x: f32, y: f32, element: *mut *const std::ffi::c_void) -> i32;
    fn AXUIElementCopyAttributeValue(element: *const std::ffi::c_void, attribute: *const std::ffi::c_void, value: *mut *const std::ffi::c_void) -> i32;
    fn AXValueGetValue(value: *const std::ffi::c_void, kind: i32, out: *mut std::ffi::c_void) -> bool;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(allocator: *const std::ffi::c_void, text: *const i8, encoding: u32) -> *const std::ffi::c_void;
    fn CFRelease(item: *const std::ffi::c_void);
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn mouse_event(flags: u32, dx: u32, dy: u32, data: u32, extra: usize);
}
