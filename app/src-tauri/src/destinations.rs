use std::path::Path;

#[cfg(target_os = "macos")]
pub fn share_path(window: &tauri::WebviewWindow, path: &Path) -> Result<(), String> {
    use objc2::runtime::AnyObject;
    use objc2::{msg_send, ClassType};
    use objc2_app_kit::{NSSharingServicePicker, NSWindow};
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_foundation::{NSArray, NSString, NSURL};

    let raw = window.ns_window().map_err(|err| err.to_string())?;
    let Some(ptr) = std::ptr::NonNull::new(raw as *mut NSWindow) else {
        return Err("This window is not available.".into());
    };
    let view = unsafe { ptr.as_ref().contentView() }.ok_or("This window has no content view.")?;
    let text = path.to_str().ok_or("That file path is not valid.")?;
    let file = NSString::from_str(text);
    let url = NSURL::fileURLWithPath(&file);
    let items = NSArray::from_slice(&[url.as_ref()]);
    let picker = unsafe {
        let allocated: *mut AnyObject = msg_send![NSSharingServicePicker::class(), alloc];
        NSSharingServicePicker::initWithItems(std::mem::transmute(allocated), &items)
    };
    let rect = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(1.0, 1.0));
    picker.showRelativeToRect_ofView_preferredEdge(rect, &view, objc2_foundation::NSRectEdge::MinY);
    std::mem::forget(picker);
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn share_path(window: &tauri::WebviewWindow, path: &Path) -> Result<(), String> {
    let _ = window;
    shotsmith_platform::share_file_windows(path).map_err(|err| err.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn share_path(window: &tauri::WebviewWindow, path: &Path) -> Result<(), String> {
    let _ = (window, path);
    Err("Share is not available on this system.".into())
}

#[cfg(target_os = "macos")]
pub fn print_path(window: &tauri::WebviewWindow, path: &Path) -> Result<(), String> {
    use objc2::runtime::AnyObject;
    use objc2::{msg_send, ClassType};
    use objc2_app_kit::{NSImage, NSImageView, NSPrintOperation};
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_foundation::NSString;

    let _ = window;
    let text = path.to_str().ok_or("That file path is not valid.")?;
    let file = NSString::from_str(text);
    let image = unsafe {
        let allocated: *mut AnyObject = msg_send![NSImage::class(), alloc];
        NSImage::initWithContentsOfFile(std::mem::transmute(allocated), &file)
    }
    .ok_or("Could not read the image for printing.")?;
    let size = image.size();
    let view = unsafe {
        let allocated: *mut AnyObject = msg_send![NSImageView::class(), alloc];
        NSImageView::initWithFrame(
            std::mem::transmute(allocated),
            CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(size.width, size.height)),
        )
    };
    view.setImage(Some(&image));
    let operation = NSPrintOperation::printOperationWithView(&view);
    if !operation.runOperation() {
        return Err("Print was cancelled.".into());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn print_path(window: &tauri::WebviewWindow, path: &Path) -> Result<(), String> {
    let _ = window;
    shotsmith_platform::print_file_windows(path).map_err(|err| err.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn print_path(window: &tauri::WebviewWindow, path: &Path) -> Result<(), String> {
    let _ = (window, path);
    Err("Print is not available on this system.".into())
}
