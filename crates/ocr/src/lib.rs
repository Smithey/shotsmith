mod secrets;

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use secrets::{scan, Finding, SecretKind};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextLine {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Error)]
pub enum OcrError {
    #[error("{0}")]
    Message(String),
}

pub fn recognize(image: &RgbaImage) -> Result<Vec<TextLine>, OcrError> {
    #[cfg(target_os = "macos")]
    {
        return mac::recognize(image);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::recognize(image);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = image;
        Err(OcrError::Message("Text recognition is not available on this system.".into()))
    }
}

pub fn plain_text(lines: &[TextLine]) -> String {
    lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n")
}

#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "windows")]
mod windows;
