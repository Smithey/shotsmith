use std::ptr::{addr_of_mut, copy_nonoverlapping, null_mut};

use windows::Graphics::Imaging::{BitmapBufferAccessMode, BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::WinRT::IMemoryBufferByteAccess;
use windows::core::{Error as WinError, Interface};

use crate::{OcrError, TextLine};

pub fn recognize(image: &image::RgbaImage) -> Result<Vec<TextLine>, OcrError> {
    let rgba = image.as_raw();
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let initialized = hr.is_ok();
    if !initialized && hr != RPC_E_CHANGED_MODE {
        return Err(OcrError::Message(format!("Could not start Windows text recognition: {}", WinError::from(hr))));
    }
    let result = read_lines(rgba, image.width(), image.height());
    if initialized {
        unsafe { CoUninitialize() };
    }
    result
}

fn read_lines(rgba: &[u8], width: u32, height: u32) -> Result<Vec<TextLine>, OcrError> {
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|err| OcrError::Message(format!("Windows text recognition is not available: {err}")))?;
    let bitmap = SoftwareBitmap::Create(BitmapPixelFormat::Rgba8, width as i32, height as i32)
        .map_err(|err| OcrError::Message(err.to_string()))?;
    let buffer = bitmap.LockBuffer(BitmapBufferAccessMode::Write).map_err(|err| OcrError::Message(err.to_string()))?;
    let reference = buffer.CreateReference().map_err(|err| OcrError::Message(err.to_string()))?;
    let access: IMemoryBufferByteAccess = reference.cast().map_err(|err| OcrError::Message(err.to_string()))?;
    let mut data: *mut u8 = null_mut();
    let mut capacity: u32 = 0;
    unsafe { access.GetBuffer(addr_of_mut!(data), addr_of_mut!(capacity)) }.map_err(|err| OcrError::Message(err.to_string()))?;
    if data.is_null() || capacity as usize != rgba.len() {
        return Err(OcrError::Message("Windows could not read the capture.".into()));
    }
    unsafe { copy_nonoverlapping(rgba.as_ptr(), data, rgba.len()) };
    drop(access);
    drop(reference);
    drop(buffer);
    let result = engine.RecognizeAsync(&bitmap).and_then(|op| op.join()).map_err(|err| OcrError::Message(err.to_string()))?;
    let lines = result.Lines().map_err(|err| OcrError::Message(err.to_string()))?;
    let mut found = Vec::new();
    for line in lines {
        let text = line.Text().map(|value| value.to_string()).unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        let words = line.Words().ok();
        let mut x0 = f32::MAX;
        let mut y0 = f32::MAX;
        let mut x1 = 0.0f32;
        let mut y1 = 0.0f32;
        if let Some(words) = words {
            for word in words {
                if let Ok(rect) = word.BoundingRect() {
                    x0 = x0.min(rect.X);
                    y0 = y0.min(rect.Y);
                    x1 = x1.max(rect.X + rect.Width);
                    y1 = y1.max(rect.Y + rect.Height);
                }
            }
        }
        if !x0.is_finite() || x1 <= x0 {
            x0 = 0.0;
            y0 = 0.0;
            x1 = width as f32;
            y1 = 24.0;
        }
        found.push(TextLine { text, x: x0, y: y0, w: (x1 - x0).max(1.0), h: (y1 - y0).max(1.0) });
    }
    Ok(found)
}
