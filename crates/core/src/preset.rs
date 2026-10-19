use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum ImageFormat {
    Png,
    Jpeg,
    Heic,
    Webp,
    Tiff,
    Pdf,
}

impl ImageFormat {
    pub fn lossless(self) -> bool {
        matches!(self, Self::Png | Self::Tiff)
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Heic => "heic",
            Self::Webp => "webp",
            Self::Tiff => "tiff",
            Self::Pdf => "pdf",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum OutputScale {
    Points,
    Native,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum AfterSaveAction {
    CopyImage,
    CopyPath,
    Reveal,
    OpenEditor,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum Effect {
    Shadow,
    Border,
    TornEdges,
    Rounded,
    Grayscale,
    Invert,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub format: ImageFormat,
    pub quality: f32,
    pub scale: OutputScale,
    pub folder: Option<String>,
    pub filename_pattern: String,
    pub after_save: Vec<AfterSaveAction>,
    pub effects: Vec<Effect>,
    pub strip_metadata: bool,
    pub donut_slot: Option<u8>,
    pub shortcut: Option<String>,
    pub builtin: bool,
    pub include_cursor: bool,
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn builtin_presets(default_folder: &str) -> Vec<Preset> {
    let pattern = "{app}-{yyyy}-{MM}-{dd} {HH}.{mm}.{ss}";
    vec![
        Preset {
            id: "builtin-docs-png".into(),
            name: "Docs PNG".into(),
            format: ImageFormat::Png,
            quality: 1.0,
            scale: OutputScale::Points,
            folder: Some(default_folder.into()),
            filename_pattern: pattern.into(),
            after_save: vec![],
            effects: vec![],
            strip_metadata: true,
            donut_slot: Some(7),
            shortcut: None,
            builtin: true,
            include_cursor: false,
        },
        Preset {
            id: "builtin-clipboard".into(),
            name: "Clipboard only".into(),
            format: ImageFormat::Png,
            quality: 1.0,
            scale: OutputScale::Native,
            folder: None,
            filename_pattern: pattern.into(),
            after_save: vec![AfterSaveAction::CopyImage],
            effects: vec![],
            strip_metadata: true,
            donut_slot: None,
            shortcut: None,
            builtin: true,
            include_cursor: false,
        },
        Preset {
            id: "builtin-bug".into(),
            name: "Bug report".into(),
            format: ImageFormat::Jpeg,
            quality: 0.8,
            scale: OutputScale::Native,
            folder: Some(default_folder.into()),
            filename_pattern: pattern.into(),
            after_save: vec![],
            effects: vec![Effect::Border],
            strip_metadata: true,
            donut_slot: None,
            shortcut: None,
            builtin: true,
            include_cursor: true,
        },
        Preset {
            id: "builtin-webp".into(),
            name: "Retina WebP".into(),
            format: ImageFormat::Webp,
            quality: 0.9,
            scale: OutputScale::Native,
            folder: Some(default_folder.into()),
            filename_pattern: pattern.into(),
            after_save: vec![],
            effects: vec![],
            strip_metadata: true,
            donut_slot: None,
            shortcut: None,
            builtin: true,
            include_cursor: false,
        },
        Preset {
            id: "builtin-pdf".into(),
            name: "Print PDF".into(),
            format: ImageFormat::Pdf,
            quality: 1.0,
            scale: OutputScale::Points,
            folder: Some(default_folder.into()),
            filename_pattern: pattern.into(),
            after_save: vec![],
            effects: vec![],
            strip_metadata: true,
            donut_slot: None,
            shortcut: None,
            builtin: true,
            include_cursor: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_round_trip() {
        let presets = builtin_presets("/tmp/Shotsmith");
        let json = serde_json::to_string(&presets).unwrap();
        let back: Vec<Preset> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, presets);
        assert!(back[0].format.lossless());
        assert!(!back[2].format.lossless());
        assert!((back[2].quality - 0.8).abs() < f32::EPSILON);
    }
}
