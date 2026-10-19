use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const DOCUMENT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub objects: Vec<AnnotationObject>,
}

impl Document {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            version: DOCUMENT_VERSION,
            width,
            height,
            objects: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AnnotationObject {
    Rect(RectObject),
    Ellipse(EllipseObject),
    Line(LineObject),
    Arrow(ArrowObject),
    Freehand(PathObject),
    Text(TextObject),
    Counter(CounterObject),
    Highlight(HighlightObject),
    Obfuscate(ObfuscateObject),
    Image(ImageObject),
    Cursor(ImageObject),
}

impl AnnotationObject {
    pub fn id(&self) -> &str {
        match self {
            Self::Rect(o) => &o.id,
            Self::Ellipse(o) => &o.id,
            Self::Line(o) => &o.id,
            Self::Arrow(o) => &o.id,
            Self::Freehand(o) => &o.id,
            Self::Text(o) => &o.id,
            Self::Counter(o) => &o.id,
            Self::Highlight(o) => &o.id,
            Self::Obfuscate(o) => &o.id,
            Self::Image(o) | Self::Cursor(o) => &o.id,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct RectObject {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub stroke: String,
    pub stroke_width: f32,
    pub fill: String,
    pub radius: f32,
    pub opacity: f32,
    pub shadow: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct EllipseObject {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub stroke: String,
    pub stroke_width: f32,
    pub fill: String,
    pub opacity: f32,
    pub shadow: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct LineObject {
    pub id: String,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stroke: String,
    pub stroke_width: f32,
    pub shadow: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum ArrowHeads {
    End,
    Both,
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ArrowObject {
    pub id: String,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub cx: Option<f32>,
    pub cy: Option<f32>,
    pub heads: ArrowHeads,
    pub stroke: String,
    pub stroke_width: f32,
    pub shadow: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct PathObject {
    pub id: String,
    pub points: Vec<Point>,
    pub stroke: String,
    pub stroke_width: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct TextObject {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub size: f32,
    pub color: String,
    pub fill: String,
    pub bold: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum CounterStyle {
    Circle,
    Square,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct CounterObject {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub number: u32,
    pub style: CounterStyle,
    pub fill: String,
    pub size: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum HighlightMode {
    Marker,
    Area,
    Grayscale,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct HighlightObject {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub mode: HighlightMode,
    pub color: String,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum ObfuscateMode {
    Pixelate,
    Blur,
    Solid,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ObfuscateObject {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub mode: ObfuscateMode,
    pub block_size: u32,
    pub color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ImageObject {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub png_base64: String,
}

pub fn next_counter(doc: &Document) -> u32 {
    doc.objects
        .iter()
        .filter_map(|o| match o {
            AnnotationObject::Counter(c) => Some(c.number),
            _ => None,
        })
        .max()
        .unwrap_or(0)
        + 1
}

pub fn renumber_counters(doc: &mut Document) {
    let mut n = 1u32;
    for object in &mut doc.objects {
        if let AnnotationObject::Counter(counter) = object {
            counter.number = n;
            n += 1;
        }
    }
}

pub fn delete_object(doc: &mut Document, id: &str) -> bool {
    let before = doc.objects.len();
    let was_counter = doc
        .objects
        .iter()
        .any(|o| o.id() == id && matches!(o, AnnotationObject::Counter(_)));
    doc.objects.retain(|o| o.id() != id);
    if was_counter {
        renumber_counters(doc);
    }
    doc.objects.len() != before
}

pub fn crop_document(doc: &mut Document, x: f32, y: f32, w: f32, h: f32) {
    doc.width = w.max(1.0).round() as u32;
    doc.height = h.max(1.0).round() as u32;
    for object in &mut doc.objects {
        shift_object(object, -x, -y);
    }
}

fn shift_object(object: &mut AnnotationObject, dx: f32, dy: f32) {
    match object {
        AnnotationObject::Rect(o) => {
            o.x += dx;
            o.y += dy;
        }
        AnnotationObject::Ellipse(o) => {
            o.x += dx;
            o.y += dy;
        }
        AnnotationObject::Line(o) => {
            o.x1 += dx;
            o.y1 += dy;
            o.x2 += dx;
            o.y2 += dy;
        }
        AnnotationObject::Arrow(o) => {
            o.x1 += dx;
            o.y1 += dy;
            o.x2 += dx;
            o.y2 += dy;
            if let Some(v) = o.cx.as_mut() {
                *v += dx;
            }
            if let Some(v) = o.cy.as_mut() {
                *v += dy;
            }
        }
        AnnotationObject::Freehand(o) => {
            for p in &mut o.points {
                p.x += dx;
                p.y += dy;
            }
        }
        AnnotationObject::Text(o) => {
            o.x += dx;
            o.y += dy;
        }
        AnnotationObject::Counter(o) => {
            o.x += dx;
            o.y += dy;
        }
        AnnotationObject::Highlight(o) => {
            o.x += dx;
            o.y += dy;
        }
        AnnotationObject::Obfuscate(o) => {
            o.x += dx;
            o.y += dy;
        }
        AnnotationObject::Image(o) | AnnotationObject::Cursor(o) => {
            o.x += dx;
            o.y += dy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter(id: &str, number: u32) -> AnnotationObject {
        AnnotationObject::Counter(CounterObject {
            id: id.into(),
            x: 0.0,
            y: 0.0,
            number,
            style: CounterStyle::Circle,
            fill: "#FF3B30".into(),
            size: 28.0,
        })
    }

    #[test]
    fn deleting_a_counter_renumbers_the_rest() {
        let mut doc = Document::new(100, 100);
        doc.objects = vec![counter("a", 1), counter("b", 2), counter("c", 3)];
        assert!(delete_object(&mut doc, "b"));
        let numbers: Vec<u32> = doc
            .objects
            .iter()
            .map(|o| match o {
                AnnotationObject::Counter(c) => c.number,
                _ => 0,
            })
            .collect();
        assert_eq!(numbers, vec![1, 2]);
    }

    #[test]
    fn export_typescript_bindings() {
        Document::export_all().expect("bindings");
    }
}
