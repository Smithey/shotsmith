use shotsmith_core::Document;

/// Editor hooks. The free app ships no built-in editors.
pub trait EditorCore: Send + Sync {
    fn id(&self) -> &'static str;
    fn before_export(&self, _document: &mut Document) {}
}

/// Capture hooks. The free app ships no built-in capture hooks.
pub trait CaptureKit: Send + Sync {
    fn id(&self) -> &'static str;
    fn after_capture(&self, _capture_id: &str) {}
}

/// History hooks. The free app ships no built-in history providers.
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

/// The free app installs nothing.
pub fn install_builtin() -> Host {
    Host {
        editors: Vec::new(),
        capture: Vec::new(),
        history: Vec::new(),
    }
}
