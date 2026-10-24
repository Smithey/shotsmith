use std::ffi::{c_void, CStr};
use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use image::RgbaImage;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGDataProvider, CGDirectDisplayID, CGDisplayBounds, CGDisplayIsBuiltin, CGDisplayIsMain,
    CGDisplayModelNumber, CGDisplayPixelsHigh, CGDisplayPixelsWide, CGError,
    CGGetActiveDisplayList, CGImage,
};
use objc2_foundation::{NSArray, NSString};

use crate::{
    bgra_to_rgba, region_points, DisplayInfo, FrozenDisplay, PlatformError, PlatformResult,
};

const CAPTURE_WAIT: Duration = Duration::from_secs(4);

pub(crate) fn on_main_thread() -> bool {
    unsafe { pthread_main_np() != 0 }
}

pub(crate) fn display_list() -> PlatformResult<Vec<DisplayInfo>> {
    let mut ids = [0u32; 16];
    let mut count = 0u32;
    let err = unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count) };
    if err != CGError::Success {
        return Err(PlatformError::Message("No displays were found.".into()));
    }
    let mut displays = Vec::new();
    for id in ids.into_iter().take(count as usize) {
        if let Some(info) = display_info(id) {
            displays.push(info);
        }
    }
    Ok(displays)
}

pub(crate) fn freeze_all() -> PlatformResult<Vec<FrozenDisplay>> {
    ensure_off_main()?;
    let listed = display_list()?;
    let content = shareable_content()?;
    let mut frozen = Vec::new();
    for info in listed {
        let Some(display) = find_display(&content, info.id) else {
            continue;
        };
        let pixels = pixel_size(info.id);
        let image = screenshot(&content, &display, None, pixels.0, pixels.1)?;
        frozen.push(FrozenDisplay { info, image });
    }
    if frozen.is_empty() {
        return Err(PlatformError::Message("No displays were captured.".into()));
    }
    Ok(frozen)
}

pub(crate) fn capture_region(
    display_id: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> PlatformResult<RgbaImage> {
    ensure_off_main()?;
    if width == 0 || height == 0 {
        return Err(PlatformError::Message("That region is empty.".into()));
    }
    let content = shareable_content()?;
    let display = find_display(&content, display_id)
        .ok_or_else(|| PlatformError::Message("That display is not available.".into()))?;
    let bounds = CGDisplayBounds(display_id);
    let pixels = pixel_size(display_id);
    let (rx, ry, rw, rh) = region_points(
        x,
        y,
        width,
        height,
        pixels.0 as u32,
        pixels.1 as u32,
        bounds.size.width,
        bounds.size.height,
    );
    let rect = CGRect::new(CGPoint::new(rx, ry), CGSize::new(rw, rh));
    screenshot(
        &content,
        &display,
        Some(rect),
        width as usize,
        height as usize,
    )
}

pub(crate) fn capture_window(window_id: u32) -> PlatformResult<RgbaImage> {
    ensure_off_main()?;
    let content = shareable_content()?;
    let window = find_window(&content, window_id)
        .ok_or_else(|| PlatformError::Message(format!("Window {window_id} is not available.")))?;
    let frame: CGRect = unsafe { msg_send![&*window, frame] };
    let filter = window_filter(&window)?;
    let scale: f32 = unsafe { msg_send![&*filter, pointPixelScale] };
    let scale = if scale.is_finite() && scale > 0.0 {
        scale as f64
    } else {
        1.0
    };
    let width = (frame.size.width * scale).round().max(1.0) as usize;
    let height = (frame.size.height * scale).round().max(1.0) as usize;
    capture_filter(&filter, None, width, height)
}

fn ensure_off_main() -> PlatformResult<()> {
    if on_main_thread() {
        Err(PlatformError::Message(
            "Screen capture cannot run on the main thread.".into(),
        ))
    } else {
        Ok(())
    }
}

fn display_info(id: CGDirectDisplayID) -> Option<DisplayInfo> {
    let bounds = CGDisplayBounds(id);
    if bounds.size.width < 1.0 || bounds.size.height < 1.0 {
        return None;
    }
    let (pixel_w, _) = pixel_size(id);
    let scale = (pixel_w as f32 / bounds.size.width as f32).max(1.0);
    let name = if CGDisplayIsBuiltin(id) {
        "Built-in Display".to_string()
    } else if CGDisplayIsMain(id) {
        "Main Display".to_string()
    } else {
        format!("Display {}", CGDisplayModelNumber(id))
    };
    Some(DisplayInfo {
        id,
        name,
        x: bounds.origin.x as i32,
        y: bounds.origin.y as i32,
        width: bounds.size.width.round().max(1.0) as u32,
        height: bounds.size.height.round().max(1.0) as u32,
        scale,
        primary: CGDisplayIsMain(id),
    })
}

fn pixel_size(id: CGDirectDisplayID) -> (usize, usize) {
    (
        CGDisplayPixelsWide(id).max(1),
        CGDisplayPixelsHigh(id).max(1),
    )
}

fn shareable_content() -> PlatformResult<Retained<AnyObject>> {
    load_framework();
    let class = cls(c"SCShareableContent")?;
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |content: *mut AnyObject, error: *mut AnyObject| {
        if !error.is_null() {
            let _ = tx.send(Err(error_text(error)));
            return;
        }
        let Some(content) = (unsafe { Retained::retain(content) }) else {
            let _ = tx.send(Err("Screen capture returned nothing.".into()));
            return;
        };
        let _ = tx.send(Ok(SendObj(Retained::into_raw(content))));
    });
    unsafe {
        let _: () = msg_send![class, getShareableContentWithCompletionHandler: &*block];
    }
    match rx.recv_timeout(CAPTURE_WAIT) {
        Ok(Ok(held)) => unsafe { Retained::from_raw(held.0) }
            .ok_or_else(|| PlatformError::Message("Screen capture returned nothing.".into())),
        Ok(Err(message)) => Err(PlatformError::Message(message)),
        Err(_) => Err(PlatformError::Message(
            "The screen capture did not finish. Try again.".into(),
        )),
    }
}

fn screenshot(
    content: &AnyObject,
    display: &AnyObject,
    source: Option<CGRect>,
    width: usize,
    height: usize,
) -> PlatformResult<RgbaImage> {
    let filter = display_filter(content, display)?;
    capture_filter(&filter, source, width, height)
}

fn display_filter(content: &AnyObject, display: &AnyObject) -> PlatformResult<Retained<AnyObject>> {
    let class = cls(c"SCContentFilter")?;
    let excluded = our_app(content);
    let excepting = NSArray::<AnyObject>::new();
    let allocated: *mut AnyObject = unsafe { msg_send![class, alloc] };
    let filter: *mut AnyObject = unsafe {
        msg_send![allocated, initWithDisplay: display, excludingApplications: &*excluded, exceptingWindows: &*excepting]
    };
    unsafe { Retained::from_raw(filter) }
        .ok_or_else(|| PlatformError::Message("Could not prepare the screen capture.".into()))
}

fn window_filter(window: &AnyObject) -> PlatformResult<Retained<AnyObject>> {
    let class = cls(c"SCContentFilter")?;
    let allocated: *mut AnyObject = unsafe { msg_send![class, alloc] };
    let filter: *mut AnyObject =
        unsafe { msg_send![allocated, initWithDesktopIndependentWindow: window] };
    unsafe { Retained::from_raw(filter) }
        .ok_or_else(|| PlatformError::Message("Could not prepare the window capture.".into()))
}

fn capture_filter(
    filter: &AnyObject,
    source: Option<CGRect>,
    width: usize,
    height: usize,
) -> PlatformResult<RgbaImage> {
    load_framework();
    let class = cls(c"SCStreamConfiguration")?;
    let config: *mut AnyObject = unsafe { msg_send![class, new] };
    let Some(config) = (unsafe { Retained::from_raw(config) }) else {
        return Err(PlatformError::Message(
            "Could not prepare the screen capture.".into(),
        ));
    };
    unsafe {
        let _: () = msg_send![&*config, setWidth: width];
        let _: () = msg_send![&*config, setHeight: height];
        let _: () = msg_send![&*config, setShowsCursor: false];
        let _: () = msg_send![&*config, setCapturesAudio: false];
        let _: () = msg_send![&*config, setPixelFormat: u32::from_be_bytes(*b"BGRA")];
        if let Some(rect) = source {
            let _: () = msg_send![&*config, setSourceRect: rect];
        }
    }
    let shot = cls(c"SCScreenshotManager")?;
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |image: *mut CGImage, error: *mut AnyObject| {
        if !error.is_null() {
            let _ = tx.send(Err(error_text(error)));
            return;
        }
        if image.is_null() {
            let _ = tx.send(Err("Screen capture returned nothing.".into()));
            return;
        }
        let result = image_from_cg(unsafe { &*image }).map_err(|err| err.to_string());
        let _ = tx.send(result);
    });
    unsafe {
        let _: () = msg_send![shot, captureImageWithFilter: filter, configuration: &*config, completionHandler: &*block];
    }
    match rx.recv_timeout(CAPTURE_WAIT) {
        Ok(Ok(image)) => Ok(image),
        Ok(Err(message)) => Err(PlatformError::Message(message)),
        Err(_) => Err(PlatformError::Message(
            "The screen capture did not finish. Try again.".into(),
        )),
    }
}

fn our_app(content: &AnyObject) -> Retained<NSArray<AnyObject>> {
    let apps: Retained<NSArray<AnyObject>> = unsafe { msg_send![content, applications] };
    let pid = std::process::id() as i32;
    for index in 0..apps.count() {
        let app = apps.objectAtIndex(index);
        let app_pid: i32 = unsafe { msg_send![&*app, processID] };
        if app_pid == pid {
            return NSArray::from_slice(&[&*app]);
        }
    }
    NSArray::new()
}

fn find_display(content: &AnyObject, id: u32) -> Option<Retained<AnyObject>> {
    let displays: Retained<NSArray<AnyObject>> = unsafe { msg_send![content, displays] };
    for index in 0..displays.count() {
        let display = displays.objectAtIndex(index);
        let display_id: u32 = unsafe { msg_send![&*display, displayID] };
        if display_id == id {
            return Some(display);
        }
    }
    None
}

fn find_window(content: &AnyObject, id: u32) -> Option<Retained<AnyObject>> {
    let windows: Retained<NSArray<AnyObject>> = unsafe { msg_send![content, windows] };
    for index in 0..windows.count() {
        let window = windows.objectAtIndex(index);
        let window_id: u32 = unsafe { msg_send![&*window, windowID] };
        if window_id == id {
            return Some(window);
        }
    }
    None
}

fn image_from_cg(image: &CGImage) -> PlatformResult<RgbaImage> {
    let width = CGImage::width(Some(image));
    let height = CGImage::height(Some(image));
    let bytes_per_row = CGImage::bytes_per_row(Some(image));
    let provider = CGImage::data_provider(Some(image))
        .ok_or_else(|| PlatformError::Message("The screen capture was empty.".into()))?;
    let data = CGDataProvider::data(Some(&provider))
        .ok_or_else(|| PlatformError::Message("The screen capture was empty.".into()))?;
    let len = data.length();
    if len <= 0 || data.byte_ptr().is_null() {
        return Err(PlatformError::Message(
            "The screen capture was empty.".into(),
        ));
    }
    let bytes = unsafe { std::slice::from_raw_parts(data.byte_ptr(), len as usize) };
    bgra_to_rgba(width, height, bytes_per_row, bytes)
}

fn error_text(error: *mut AnyObject) -> String {
    if error.is_null() {
        return "Screen capture failed.".into();
    }
    let text: *mut NSString = unsafe { msg_send![error, localizedDescription] };
    if text.is_null() {
        "Screen capture failed.".into()
    } else {
        unsafe { &*text }.to_string()
    }
}

fn cls(name: &CStr) -> PlatformResult<&'static AnyClass> {
    AnyClass::get(name)
        .ok_or_else(|| PlatformError::Message("Screen capture is not available.".into()))
}

fn load_framework() {
    #[link(name = "ScreenCaptureKit", kind = "framework")]
    extern "C" {
        #[link_name = "OBJC_CLASS_$_SCShareableContent"]
        static CLASS: *const c_void;
    }
    unsafe {
        std::ptr::read_volatile(&CLASS);
    }
}

struct SendObj(*mut AnyObject);
unsafe impl Send for SendObj {}

#[link(name = "pthread")]
extern "C" {
    fn pthread_main_np() -> i32;
}
