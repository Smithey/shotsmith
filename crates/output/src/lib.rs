use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use shotsmith_core::{Document, ImageFormat};
use std::io::Cursor;
use std::path::Path;
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug, Error)]
pub enum OutputError {
    #[error("{0}")]
    Message(String),
}

pub fn encode_png_fast(image: &RgbaImage) -> Result<Vec<u8>, OutputError> {
    let mut bytes = Vec::new();
    PngEncoder::new_with_quality(&mut bytes, CompressionType::Fast, FilterType::NoFilter)
        .write_image(image.as_raw(), image.width(), image.height(), ExtendedColorType::Rgba8)
        .map_err(|err| OutputError::Message(err.to_string()))?;
    Ok(bytes)
}

pub fn encode(image: &RgbaImage, format: ImageFormat, quality: f32) -> Result<Vec<u8>, OutputError> {
    match format {
        ImageFormat::Png => encode_png(image),
        ImageFormat::Jpeg => encode_jpeg(image, quality),
        ImageFormat::Webp => encode_with(image, |buf, img| {
            image::codecs::webp::WebPEncoder::new_lossless(buf)
                .write_image(img.as_raw(), img.width(), img.height(), ExtendedColorType::Rgba8)
                .map_err(|err| OutputError::Message(err.to_string()))
        }),
        ImageFormat::Tiff => encode_with(image, |buf, img| {
            image::codecs::tiff::TiffEncoder::new(buf)
                .write_image(img.as_raw(), img.width(), img.height(), ExtendedColorType::Rgba8)
                .map_err(|err| OutputError::Message(err.to_string()))
        }),
        ImageFormat::Pdf => encode_pdf(image),
        ImageFormat::Heic => Err(OutputError::Message(
            "HEIC export needs a platform encoder, which is not available in this build.".into(),
        )),
    }
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), OutputError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| OutputError::Message(err.to_string()))?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|ext| ext.to_str()).unwrap_or("part")
    ));
    std::fs::write(&tmp, bytes).map_err(|err| OutputError::Message(err.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|err| OutputError::Message(err.to_string()))?;
    Ok(())
}

pub fn write_shotsmith(path: &Path, doc: &Document, original_png: &[u8]) -> Result<(), OutputError> {
    let json = serde_json::to_vec_pretty(doc).map_err(|err| OutputError::Message(err.to_string()))?;
    let file = std::fs::File::create(path).map_err(|err| OutputError::Message(err.to_string()))?;
    let mut zip = ZipWriter::new(file);
    let json_options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let png_options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("document.json", json_options)
        .map_err(|err| OutputError::Message(err.to_string()))?;
    std::io::Write::write_all(&mut zip, &json).map_err(|err| OutputError::Message(err.to_string()))?;
    zip.start_file("original.png", png_options)
        .map_err(|err| OutputError::Message(err.to_string()))?;
    std::io::Write::write_all(&mut zip, original_png)
        .map_err(|err| OutputError::Message(err.to_string()))?;
    zip.finish().map_err(|err| OutputError::Message(err.to_string()))?;
    Ok(())
}

pub fn read_shotsmith(path: &Path) -> Result<(Document, Vec<u8>), OutputError> {
    let file = std::fs::File::open(path).map_err(|err| OutputError::Message(err.to_string()))?;
    let mut archive = ZipArchive::new(file).map_err(|err| OutputError::Message(err.to_string()))?;
    let mut json = String::new();
    std::io::Read::read_to_string(&mut archive.by_name("document.json").map_err(|err| OutputError::Message(err.to_string()))?, &mut json)
        .map_err(|err| OutputError::Message(err.to_string()))?;
    let doc: Document = serde_json::from_str(&json).map_err(|err| OutputError::Message(err.to_string()))?;
    let mut png = Vec::new();
    std::io::Read::read_to_end(&mut archive.by_name("original.png").map_err(|err| OutputError::Message(err.to_string()))?, &mut png)
        .map_err(|err| OutputError::Message(err.to_string()))?;
    Ok((doc, png))
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, OutputError> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(image.as_raw(), image.width(), image.height(), ExtendedColorType::Rgba8)
        .map_err(|err| OutputError::Message(err.to_string()))?;
    Ok(bytes)
}

fn encode_jpeg(image: &RgbaImage, quality: f32) -> Result<Vec<u8>, OutputError> {
    let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
    let mut bytes = Vec::new();
    let q = (quality.clamp(0.01, 1.0) * 100.0).round() as u8;
    JpegEncoder::new_with_quality(&mut bytes, q)
        .write_image(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
        .map_err(|err| OutputError::Message(err.to_string()))?;
    Ok(bytes)
}

fn encode_with(
    image: &RgbaImage,
    write: impl FnOnce(&mut Cursor<Vec<u8>>, &RgbaImage) -> Result<(), OutputError>,
) -> Result<Vec<u8>, OutputError> {
    let mut cursor = Cursor::new(Vec::new());
    write(&mut cursor, image)?;
    Ok(cursor.into_inner())
}

fn encode_pdf(image: &RgbaImage) -> Result<Vec<u8>, OutputError> {
    let jpeg = encode_jpeg(image, 0.92)?;
    let width = image.width();
    let height = image.height();
    let mut content = format!(
        "q\n{width} 0 0 {height} 0 0 cm\n/Im0 Do\nQ\n"
    );
    let content_bytes = content.as_bytes();
    let mut objects: Vec<Vec<u8>> = Vec::new();
    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objects.push(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec());
    objects.push(format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Contents 4 0 R /Resources << /XObject << /Im0 5 0 R >> >> >>"
    ).into_bytes());
    let mut stream = format!("<< /Length {} >>\nstream\n", content_bytes.len()).into_bytes();
    stream.extend_from_slice(content_bytes);
    stream.extend_from_slice(b"\nendstream");
    objects.push(stream);
    let mut image_obj = format!(
        "<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
        jpeg.len()
    ).into_bytes();
    image_obj.extend_from_slice(&jpeg);
    image_obj.extend_from_slice(b"\nendstream");
    objects.push(image_obj);

    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        pdf.extend_from_slice(object);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
            objects.len() + 1
        )
        .as_bytes(),
    );
    let _ = content;
    Ok(pdf)
}
