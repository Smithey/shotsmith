use std::ptr::null;

use objc2::rc::autoreleasepool;
use objc2::{msg_send, ClassType};
use objc2::runtime::AnyObject;
use objc2_core_foundation::CGRect;
use objc2_vision::VNDetectedObjectObservation;
use objc2_core_foundation::{CFData, CFRetained};
use objc2_core_graphics::{
    CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo,
};
use objc2_foundation::{NSArray, NSDictionary};
use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};

use crate::{OcrError, TextLine};

pub fn recognize(image: &image::RgbaImage) -> Result<Vec<TextLine>, OcrError> {
    let rgba = image.as_raw();
    let width = image.width();
    let height = image.height();
    let cg = cg_image(rgba, width, height)?;
    autoreleasepool(|_| read_lines(&cg, width, height))
}

fn cg_image(rgba: &[u8], width: u32, height: u32) -> Result<CFRetained<CGImage>, OcrError> {
    let data = CFData::from_bytes(rgba);
    let provider = CGDataProvider::with_cf_data(Some(&data)).ok_or_else(|| OcrError::Message("Could not prepare the image for text recognition.".into()))?;
    let space = CGColorSpace::new_device_rgb().ok_or_else(|| OcrError::Message("Could not prepare a colour space.".into()))?;
    unsafe {
        CGImage::new(
            width as usize,
            height as usize,
            8,
            32,
            width as usize * 4,
            Some(&space),
            CGBitmapInfo(CGImageAlphaInfo::Last.0),
            Some(&provider),
            null(),
            false,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    }
    .ok_or_else(|| OcrError::Message("Could not build an image for text recognition.".into()))
}

fn read_lines(image: &CGImage, width: u32, height: u32) -> Result<Vec<TextLine>, OcrError> {
    let request = VNRecognizeTextRequest::new();
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
    request.setUsesLanguageCorrection(true);
    let handler = unsafe {
        let allocated: *mut AnyObject = msg_send![VNImageRequestHandler::class(), alloc];
        let options = NSDictionary::new();
        VNImageRequestHandler::initWithCGImage_options(
            std::mem::transmute(allocated),
            image,
            &options,
        )
    };
    let as_request: &VNRequest = &request;
    handler
        .performRequests_error(&NSArray::from_slice(&[as_request]))
        .map_err(|err| OcrError::Message(err.localizedDescription().to_string()))?;
    let Some(observations) = request.results() else {
        return Ok(Vec::new());
    };
    let width = width as f32;
    let height = height as f32;
    let mut lines = Vec::new();
    for observation in observations.iter() {
        let Some(candidate) = observation.topCandidates(1).firstObject() else {
            continue;
        };
        let text = candidate.string().to_string();
        if text.trim().is_empty() {
            continue;
        }
        let detected = unsafe { &*(std::ptr::from_ref(&*observation).cast::<VNDetectedObjectObservation>()) };
        let bounds: CGRect = unsafe { detected.boundingBox() };
        let w = (bounds.size.width as f32) * width;
        let h = (bounds.size.height as f32) * height;
        let x = (bounds.origin.x as f32) * width;
        let y = (1.0 - bounds.origin.y as f32) * height - h;
        lines.push(TextLine { text, x, y, w: w.max(1.0), h: h.max(1.0) });
    }
    Ok(lines)
}
