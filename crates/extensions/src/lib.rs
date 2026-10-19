use shotsmith_core::Document;

/// Editor hooks. Pro implements redaction, style kits, and extra objects later.
pub trait EditorCore: Send + Sync {
    fn id(&self) -> &'static str;
    fn before_export(&self, _document: &mut Document) {}
}

/// Capture hooks. Pro implements element snapping and step guides later.
pub trait CaptureKit: Send + Sync {
    fn id(&self) -> &'static str;
    fn after_capture(&self, _capture_id: &str) {}
}

/// History hooks. Pro implements meaning search and sync later.
pub trait HistoryProvider: Send + Sync {
    fn id(&self) -> &'static str;
}

pub struct Host {
    pub editors: Vec<Box<dyn EditorCore>>,
    pub capture: Vec<Box<dyn CaptureKit>>,
    pub history: Vec<Box<dyn HistoryProvider>>,
}

impl Host {
    pub fn before_export(&self, document: &mut Document) {
        for editor in &self.editors {
            editor.before_export(document);
        }
    }
}

/// The free app installs nothing. Official builds patch this crate.
pub fn install_builtin() -> Host {
    Host {
        editors: Vec::new(),
        capture: Vec::new(),
        history: Vec::new(),
    }
}
