use ab_glyph::{Font, FontRef, PxScale, point};
use base64::Engine;
use image::{Rgba, RgbaImage};
use shotsmith_core::{
    AnnotationObject, ArrowHeads, CounterStyle, Document, Effect, HighlightMode, MagnifyObject, ObfuscateMode,
};
use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform,
};

pub fn render(image: &RgbaImage, doc: &Document, effects: &[Effect]) -> RgbaImage {
    let mut pixmap = image_to_pixmap(image);
    for object in &doc.objects {
        draw_object(&mut pixmap, object);
    }
    apply_effects(&mut pixmap, effects);
    pixmap_to_image(&pixmap)
}

pub fn crop_image(image: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
    let x = x.min(image.width().saturating_sub(1));
    let y = y.min(image.height().saturating_sub(1));
    let w = w.max(1).min(image.width() - x);
    let h = h.max(1).min(image.height() - y);
    image::imageops::crop_imm(image, x, y, w, h).to_image()
}

fn draw_object(pixmap: &mut Pixmap, object: &AnnotationObject) {
    match object {
        AnnotationObject::Rect(o) => {
            shadow_rect(pixmap, o.x, o.y, o.w, o.h, o.radius, shade(o.shadow, o.shadow_blur, o.shadow_x, o.shadow_y, o.shadow_opacity));
            let mut paint = paint_hex(&o.fill, o.opacity);
            if o.fill != "none" {
                fill_round(pixmap, o.x, o.y, o.w, o.h, o.radius, &paint);
            }
            paint = paint_hex(&o.stroke, o.opacity);
            stroke_round(pixmap, o.x, o.y, o.w, o.h, o.radius, o.stroke_width, &paint);
        }
        AnnotationObject::Ellipse(o) => {
            shadow_oval(pixmap, o.x, o.y, o.w, o.h, shade(o.shadow, o.shadow_blur, o.shadow_x, o.shadow_y, o.shadow_opacity));
            if o.fill != "none" {
                let paint = paint_hex(&o.fill, o.opacity);
                fill_oval(pixmap, o.x, o.y, o.w, o.h, &paint);
            }
            let paint = paint_hex(&o.stroke, o.opacity);
            stroke_oval(pixmap, o.x, o.y, o.w, o.h, o.stroke_width, &paint);
        }
        AnnotationObject::Line(o) => {
            let paint = paint_hex(&o.stroke, 1.0);
            stroke_line(
                pixmap,
                o.x1,
                o.y1,
                o.x2,
                o.y2,
                None,
                o.stroke_width,
                &paint,
                shade(o.shadow, o.shadow_blur, o.shadow_x, o.shadow_y, o.shadow_opacity),
                true,
            );
        }
        AnnotationObject::Arrow(o) => {
            let paint = paint_hex(&o.stroke, 1.0);
            let control = match (o.cx, o.cy) {
                (Some(cx), Some(cy)) => Some((cx, cy)),
                _ => None,
            };
            let (hx, hy) = match control {
                Some((cx, cy)) => (2.0 * o.x2 - 2.0 * cx + o.x1, 2.0 * o.y2 - 2.0 * cy + o.y1),
                None => (o.x2 - o.x1, o.y2 - o.y1),
            };
            let inset = (o.stroke_width * 3.2).max(10.0) * 0.82;
            let end = matches!(o.heads, ArrowHeads::End | ArrowHeads::Both);
            let both = o.heads == ArrowHeads::Both;
            let (x2, y2) = if end { pull_back(o.x2, o.y2, hx, hy, inset) } else { (o.x2, o.y2) };
            let (tx, ty) = match control {
                Some((cx, cy)) => (2.0 * o.x1 - 2.0 * cx + o.x2, 2.0 * o.y1 - 2.0 * cy + o.y2),
                None => (o.x1 - o.x2, o.y1 - o.y2),
            };
            let (x1, y1) = if both { pull_back(o.x1, o.y1, tx, ty, inset) } else { (o.x1, o.y1) };
            stroke_line(
                pixmap,
                x1,
                y1,
                x2,
                y2,
                control,
                o.stroke_width,
                &paint,
                shade(o.shadow, o.shadow_blur, o.shadow_x, o.shadow_y, o.shadow_opacity),
                false,
            );
            if end {
                shadow_head(pixmap, o.x2, o.y2, hx, hy, o.stroke_width, shade(o.shadow, o.shadow_blur, o.shadow_x, o.shadow_y, o.shadow_opacity));
                draw_head(pixmap, o.x2, o.y2, hx, hy, o.stroke_width, &paint);
            }
            if both {
                shadow_head(pixmap, o.x1, o.y1, tx, ty, o.stroke_width, shade(o.shadow, o.shadow_blur, o.shadow_x, o.shadow_y, o.shadow_opacity));
                draw_head(pixmap, o.x1, o.y1, tx, ty, o.stroke_width, &paint);
            }
        }
        AnnotationObject::Freehand(o) => {
            if o.points.len() < 2 {
                return;
            }
            let mut pb = PathBuilder::new();
            pb.move_to(o.points[0].x, o.points[0].y);
            for p in o.points.iter().skip(1) {
                pb.line_to(p.x, p.y);
            }
            if let Some(path) = pb.finish() {
                let paint = paint_hex(&o.stroke, 1.0);
                let mut stroke = Stroke::default();
                stroke.width = o.stroke_width.max(1.0);
                stroke.line_cap = LineCap::Round;
                stroke.line_join = LineJoin::Round;
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
        AnnotationObject::Text(o) => draw_text(pixmap, o),
        AnnotationObject::Counter(o) => draw_counter(pixmap, o),
        AnnotationObject::Highlight(o) => draw_highlight(pixmap, o),
        AnnotationObject::Obfuscate(o) => draw_obfuscate(pixmap, o),
        AnnotationObject::Image(o) | AnnotationObject::Cursor(o) => draw_embedded(pixmap, o),
        AnnotationObject::Magnify(o) => {
            // Sample the picture so far, so a redaction drawn earlier stays covered.
            let sample = pixmap.clone();
            draw_magnify(pixmap, &sample, o);
        }
    }
}

fn draw_magnify(pixmap: &mut Pixmap, base: &Pixmap, object: &MagnifyObject) {
    let radius = (object.w.abs().min(object.h.abs()) / 2.0).max(8.0);
    let cx = object.x + object.w / 2.0;
    let cy = object.y + object.h / 2.0;
    let factor = object.factor.clamp(2.0, 4.0);
    let r = radius.round() as i32;
    let width = pixmap.width() as i32;
    let height = pixmap.height() as i32;
    let stride = pixmap.width();
    let bw = base.width() as i32;
    let bh = base.height() as i32;
    let bstride = base.width();
    let base_px = base.pixels();
    {
        let dest = pixmap.pixels_mut();
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let dst_x = (cx.round() as i32) + dx;
                let dst_y = (cy.round() as i32) + dy;
                if dst_x < 0 || dst_y < 0 || dst_x >= width || dst_y >= height {
                    continue;
                }
                let sx = (cx + dx as f32 / factor).round() as i32;
                let sy = (cy + dy as f32 / factor).round() as i32;
                if sx < 0 || sy < 0 || sx >= bw || sy >= bh {
                    continue;
                }
                let pixel = base_px[(sy as u32 * bstride + sx as u32) as usize];
                dest[(dst_y as u32 * stride + dst_x as u32) as usize] = pixel;
            }
        }
    }
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 230);
    stroke_oval(pixmap, cx - radius, cy - radius, radius * 2.0, radius * 2.0, 2.0, &paint);
}

fn draw_highlight(pixmap: &mut Pixmap, o: &shotsmith_core::HighlightObject) {
    match o.mode {
        HighlightMode::Marker => {
            let paint = paint_hex(&o.color, o.opacity);
            fill_round(pixmap, o.x, o.y, o.w, o.h, 2.0, &paint);
        }
        HighlightMode::Area => {
            let mut paint = Paint::default();
            paint.set_color_rgba8(0, 0, 0, 140);
            let (pw, ph) = (pixmap.width() as f32, pixmap.height() as f32);
            fill_if(pixmap, 0.0, 0.0, pw, o.y.max(0.0), &paint);
            fill_if(pixmap, 0.0, o.y + o.h, pw, (ph - (o.y + o.h)).max(0.0), &paint);
            fill_if(pixmap, 0.0, o.y, o.x.max(0.0), o.h, &paint);
            fill_if(
                pixmap,
                o.x + o.w,
                o.y,
                (pw - (o.x + o.w)).max(0.0),
                o.h,
                &paint,
            );
        }
        HighlightMode::Grayscale => grayscale_outside(pixmap, o.x, o.y, o.w, o.h),
    }
}

fn draw_obfuscate(pixmap: &mut Pixmap, o: &shotsmith_core::ObfuscateObject) {
    let width = pixmap.width();
    let height = pixmap.height();
    let x0 = (o.x.floor().max(0.0) as u32).min(width);
    let y0 = (o.y.floor().max(0.0) as u32).min(height);
    let x1 = ((o.x + o.w).ceil().max(0.0) as u32).min(width);
    let y1 = ((o.y + o.h).ceil().max(0.0) as u32).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    match o.mode {
        ObfuscateMode::Solid => {
            let (r, g, b, _) = parse_hex(&o.color).unwrap_or((0, 0, 0, 255));
            let data = pixmap.data_mut();
            for y in y0..y1 {
                let mut i = ((y * width + x0) * 4) as usize;
                for _ in x0..x1 {
                    data[i] = r;
                    data[i + 1] = g;
                    data[i + 2] = b;
                    data[i + 3] = 255;
                    i += 4;
                }
            }
        }
        ObfuscateMode::Pixelate => pixelate_region(pixmap, x0, y0, x1, y1, o.block_size.max(2)),
        ObfuscateMode::Blur => blur_region(pixmap, x0, y0, x1, y1, o.block_size.max(1)),
    }
}

fn pixelate_region(pixmap: &mut Pixmap, x0: u32, y0: u32, x1: u32, y1: u32, block: u32) {
    let width = pixmap.width();
    let source = pixmap.data().to_vec();
    let data = pixmap.data_mut();
    let mut y = y0;
    while y < y1 {
        let bh = (y + block).min(y1);
        let mut x = x0;
        while x < x1 {
            let bw = (x + block).min(x1);
            let mut sr = 0u64;
            let mut sg = 0u64;
            let mut sb = 0u64;
            let mut n = 0u64;
            for py in y..bh {
                let mut i = ((py * width + x) * 4) as usize;
                for _ in x..bw {
                    sr += source[i] as u64;
                    sg += source[i + 1] as u64;
                    sb += source[i + 2] as u64;
                    n += 1;
                    i += 4;
                }
            }
            let color = [(sr / n) as u8, (sg / n) as u8, (sb / n) as u8];
            for py in y..bh {
                let mut i = ((py * width + x) * 4) as usize;
                for _ in x..bw {
                    data[i] = color[0];
                    data[i + 1] = color[1];
                    data[i + 2] = color[2];
                    data[i + 3] = 255;
                    i += 4;
                }
            }
            x = bw;
        }
        y = bh;
    }
}

fn blur_region(pixmap: &mut Pixmap, x0: u32, y0: u32, x1: u32, y1: u32, radius: u32) {
    let width = pixmap.width() as i32;
    let height = pixmap.height() as i32;
    let radius = radius as i32;
    let left = (x0 as i32 - radius).max(0) as u32;
    let top = (y0 as i32 - radius).max(0) as u32;
    let right = ((x1 as i32 + radius).min(width) as u32).max(left + 1);
    let bottom = ((y1 as i32 + radius).min(height) as u32).max(top + 1);
    let full = pixmap.width();
    let rw = right - left;
    let rh = bottom - top;
    let mut src = vec![0u8; (rw * rh * 4) as usize];
    let mut dst = vec![0u8; src.len()];
    {
        let data = pixmap.data();
        for row in 0..rh {
            let from = (((top + row) * full + left) * 4) as usize;
            let to = (row * rw * 4) as usize;
            src[to..to + (rw * 4) as usize].copy_from_slice(&data[from..from + (rw * 4) as usize]);
        }
    }
    for _ in 0..3 {
        box_horizontal(&src, &mut dst, rw, rh, radius as u32);
        box_vertical(&dst, &mut src, rw, rh, radius as u32);
    }
    let data = pixmap.data_mut();
    let ox = x0 - left;
    let oy = y0 - top;
    for row in 0..(y1 - y0) {
        let from = (((oy + row) * rw + ox) * 4) as usize;
        let to = (((y0 + row) * full + x0) * 4) as usize;
        let bytes = ((x1 - x0) * 4) as usize;
        data[to..to + bytes].copy_from_slice(&src[from..from + bytes]);
    }
}

fn box_horizontal(src: &[u8], dst: &mut [u8], width: u32, height: u32, radius: u32) {
    let span = radius * 2 + 1;
    let width = width as i32;
    let radius = radius as i32;
    for y in 0..height {
        let row = y * width as u32;
        let mut r = 0u32;
        let mut g = 0u32;
        let mut b = 0u32;
        let mut a = 0u32;
        for i in -radius..=radius {
            let p = ((row + i.clamp(0, width - 1) as u32) * 4) as usize;
            r += src[p] as u32;
            g += src[p + 1] as u32;
            b += src[p + 2] as u32;
            a += src[p + 3] as u32;
        }
        for x in 0..width {
            let p = ((row + x as u32) * 4) as usize;
            dst[p] = (r / span) as u8;
            dst[p + 1] = (g / span) as u8;
            dst[p + 2] = (b / span) as u8;
            dst[p + 3] = (a / span) as u8;
            let remove = ((row + (x - radius).clamp(0, width - 1) as u32) * 4) as usize;
            let add = ((row + (x + radius + 1).clamp(0, width - 1) as u32) * 4) as usize;
            r = r + src[add] as u32 - src[remove] as u32;
            g = g + src[add + 1] as u32 - src[remove + 1] as u32;
            b = b + src[add + 2] as u32 - src[remove + 2] as u32;
            a = a + src[add + 3] as u32 - src[remove + 3] as u32;
        }
    }
}

fn box_vertical(src: &[u8], dst: &mut [u8], width: u32, height: u32, radius: u32) {
    let span = radius * 2 + 1;
    let height = height as i32;
    let radius = radius as i32;
    for x in 0..width {
        let mut r = 0u32;
        let mut g = 0u32;
        let mut b = 0u32;
        let mut a = 0u32;
        for i in -radius..=radius {
            let p = ((i.clamp(0, height - 1) as u32 * width + x) * 4) as usize;
            r += src[p] as u32;
            g += src[p + 1] as u32;
            b += src[p + 2] as u32;
            a += src[p + 3] as u32;
        }
        for y in 0..height {
            let p = ((y as u32 * width + x) * 4) as usize;
            dst[p] = (r / span) as u8;
            dst[p + 1] = (g / span) as u8;
            dst[p + 2] = (b / span) as u8;
            dst[p + 3] = (a / span) as u8;
            let remove = (((y - radius).clamp(0, height - 1) as u32 * width + x) * 4) as usize;
            let add = (((y + radius + 1).clamp(0, height - 1) as u32 * width + x) * 4) as usize;
            r = r + src[add] as u32 - src[remove] as u32;
            g = g + src[add + 1] as u32 - src[remove + 1] as u32;
            b = b + src[add + 2] as u32 - src[remove + 2] as u32;
            a = a + src[add + 3] as u32 - src[remove + 3] as u32;
        }
    }
}

fn draw_text(pixmap: &mut Pixmap, o: &shotsmith_core::TextObject) {
    let lines: Vec<&str> = if o.text.is_empty() { vec![""] } else { o.text.split('\n').collect() };
    let longest = lines.iter().map(|line| line.chars().count()).max().unwrap_or(1) as f32;
    let width = (longest * o.size * 0.55).max(o.size);
    let height = lines.len() as f32 * o.size * 1.2;
    if o.fill != "none" {
        let paint = paint_hex(if o.fill == "white" { "#FFFFFF" } else { &o.fill }, 1.0);
        fill_round(pixmap, o.x - 4.0, o.y - o.size, width + 8.0, height + 8.0, 4.0, &paint);
    }
    let font_bytes = font_bytes(o.bold);
    let Ok(font) = FontRef::try_from_slice(font_bytes) else {
        return;
    };
    let scale = PxScale::from(o.size.max(8.0));
    let (r, g, b, a) = parse_hex(&o.color).unwrap_or((29, 29, 31, 255));
    for (index, line) in lines.iter().enumerate() {
        let mut caret = point(o.x, o.y + index as f32 * o.size * 1.2);
        for ch in line.chars() {
            let glyph_id = font.glyph_id(ch);
            let glyph = glyph_id.with_scale(scale);
            if let Some(outlined) = font.outline_glyph(glyph) {
                outlined.draw(|dx, dy, cov| {
                    let x = (caret.x + outlined.px_bounds().min.x + dx as f32).round() as i32;
                    let y = (caret.y + outlined.px_bounds().min.y + dy as f32).round() as i32;
                    if x < 0 || y < 0 || x >= pixmap.width() as i32 || y >= pixmap.height() as i32 {
                        return;
                    }
                    blend_pixel(pixmap, x as u32, y as u32, r, g, b, (a as f32 * cov) as u8);
                });
            }
            let units = font.units_per_em().unwrap_or(1000.0);
            caret.x += font.h_advance_unscaled(glyph_id) as f32 * scale.x / units;
        }
    }
}

fn draw_counter(pixmap: &mut Pixmap, o: &shotsmith_core::CounterObject) {
    let paint = paint_hex(&o.fill, 1.0);
    let s = o.size.max(16.0);
    match o.style {
        CounterStyle::Circle => fill_oval(pixmap, o.x, o.y, s, s, &paint),
        CounterStyle::Square => fill_round(pixmap, o.x, o.y, s, s, 4.0, &paint),
    }
    let label = shotsmith_core::TextObject {
        id: String::new(),
        x: o.x + s * 0.28,
        y: o.y + s * 0.72,
        text: o.number.to_string(),
        size: s * 0.55,
        color: "#FFFFFF".into(),
        fill: "none".into(),
        bold: true,
    };
    draw_text(pixmap, &label);
}

fn draw_embedded(pixmap: &mut Pixmap, o: &shotsmith_core::ImageObject) {
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(o.png_base64.trim()) else {
        return;
    };
    let Ok(img) = image::load_from_memory(&bytes) else {
        return;
    };
    let rgba = img.to_rgba8();
    let src = image_to_pixmap(&rgba);
    let sx = if rgba.width() == 0 { 1.0 } else { o.w / rgba.width() as f32 };
    let sy = if rgba.height() == 0 { 1.0 } else { o.h / rgba.height() as f32 };
    pixmap.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        Transform::from_scale(sx, sy).post_translate(o.x, o.y),
        None,
    );
}

fn apply_effects(pixmap: &mut Pixmap, effects: &[Effect]) {
    if effects.is_empty() {
        return;
    }
    let mut image = pixmap_to_image(pixmap);
    for effect in effects {
        match effect {
            Effect::Grayscale => {
                for p in image.pixels_mut() {
                    let l = (p.0[0] as u16 * 30 + p.0[1] as u16 * 59 + p.0[2] as u16 * 11) / 100;
                    p.0[0] = l as u8;
                    p.0[1] = l as u8;
                    p.0[2] = l as u8;
                }
            }
            Effect::Invert => {
                for p in image.pixels_mut() {
                    p.0[0] = 255 - p.0[0];
                    p.0[1] = 255 - p.0[1];
                    p.0[2] = 255 - p.0[2];
                }
            }
            Effect::Rounded | Effect::TornEdges | Effect::Shadow | Effect::Border => {}
        }
    }
    let mut next = image_to_pixmap(&image);
    let pad = if effects.contains(&Effect::Shadow) { 36.0 } else { 0.0 };
    let bw = if effects.contains(&Effect::Border) { 8.0 } else { 0.0 };
    let grow = (pad + bw) as u32;
    if grow > 0 {
        let mut canvas = Pixmap::new(next.width() + grow * 2, next.height() + grow * 2).unwrap();
        if effects.contains(&Effect::Shadow) {
            blit_outer_shadow(
                &mut canvas,
                grow as f32,
                grow as f32,
                next.width() as f32,
                next.height() as f32,
                0.0,
                0.0,
                10.0,
                16.0,
                0.45,
                false,
            );
        }
        canvas.draw_pixmap(
            grow as i32,
            grow as i32,
            next.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        next = canvas;
    }
    if effects.contains(&Effect::Border) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(255, 255, 255, 255);
        let width = next.width() as f32 - bw;
        let height = next.height() as f32 - bw;
        stroke_round(
            &mut next,
            bw / 2.0,
            bw / 2.0,
            width,
            height,
            0.0,
            bw.max(2.0),
            &paint,
        );
    }
    if effects.contains(&Effect::Rounded) {
        next = clip_rounded(next, 18.0);
    }
    if effects.contains(&Effect::TornEdges) {
        next = tear(next);
    }
    *pixmap = next;
}

fn clip_rounded(src: Pixmap, radius: f32) -> Pixmap {
    let mut out = Pixmap::new(src.width(), src.height()).unwrap();
    let mut pb = PathBuilder::new();
    let w = src.width() as f32;
    let h = src.height() as f32;
    let r = radius.min(w / 2.0).min(h / 2.0);
    pb.move_to(r, 0.0);
    pb.line_to(w - r, 0.0);
    pb.quad_to(w, 0.0, w, r);
    pb.line_to(w, h - r);
    pb.quad_to(w, h, w - r, h);
    pb.line_to(r, h);
    pb.quad_to(0.0, h, 0.0, h - r);
    pb.line_to(0.0, r);
    pb.quad_to(0.0, 0.0, r, 0.0);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = tiny_skia::PixmapPaint::default();
        paint.quality = tiny_skia::FilterQuality::Bilinear;
        out.fill_path(
            &path,
            &paint_hex("#000", 1.0),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        let mask = out.clone();
        out.fill(tiny_skia::Color::TRANSPARENT);
        out.draw_pixmap(0, 0, src.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
        for i in (0..out.data().len()).step_by(4) {
            let a = mask.data()[i + 3];
            out.data_mut()[i + 3] = a;
        }
    }
    out
}

fn tear(src: Pixmap) -> Pixmap {
    let mut out = src;
    let w = out.width();
    let h = out.height();
    for x in 0..w {
        let n = ((x as f32 * 0.35).sin() * 4.0 + 5.0).round().max(1.0) as u32;
        for y in 0..n.min(h) {
            clear_pixel(&mut out, x, y);
            clear_pixel(&mut out, x, h - 1 - y);
        }
    }
    for y in 0..h {
        let n = ((y as f32 * 0.4).cos() * 4.0 + 5.0).round().max(1.0) as u32;
        for x in 0..n.min(w) {
            clear_pixel(&mut out, x, y);
            clear_pixel(&mut out, w - 1 - x, y);
        }
    }
    out
}

fn clear_pixel(pixmap: &mut Pixmap, x: u32, y: u32) {
    let i = ((y * pixmap.width() + x) * 4) as usize;
    let data = pixmap.data_mut();
    if i + 3 < data.len() {
        data[i] = 0;
        data[i + 1] = 0;
        data[i + 2] = 0;
        data[i + 3] = 0;
    }
}

fn grayscale_outside(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32) {
    let mut image = pixmap_to_image(pixmap);
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = (x + w).ceil() as i32;
    let y1 = (y + h).ceil() as i32;
    for py in 0..image.height() {
        for px in 0..image.width() {
            if (px as i32) >= x0 && (px as i32) < x1 && (py as i32) >= y0 && (py as i32) < y1 {
                continue;
            }
            let p = image.get_pixel_mut(px, py);
            let l = (p.0[0] as u16 * 30 + p.0[1] as u16 * 59 + p.0[2] as u16 * 11) / 100;
            p.0[0] = l as u8;
            p.0[1] = l as u8;
            p.0[2] = l as u8;
        }
    }
    *pixmap = image_to_pixmap(&image);
}

fn blend_pixel(pixmap: &mut Pixmap, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
    if a == 0 {
        return;
    }
    let i = ((y * pixmap.width() + x) * 4) as usize;
    let data = pixmap.data_mut();
    let src_a = a as u16;
    let inv = 255 - src_a;
    data[i] = ((r as u16 * src_a + data[i] as u16 * inv) / 255) as u8;
    data[i + 1] = ((g as u16 * src_a + data[i + 1] as u16 * inv) / 255) as u8;
    data[i + 2] = ((b as u16 * src_a + data[i + 2] as u16 * inv) / 255) as u8;
    data[i + 3] = 255;
}

fn font_bytes(bold: bool) -> &'static [u8] {
    // DejaVu is vendored so a Windows or Linux runner can compile without a system font.
    // License: crates/render/fonts/LICENSE
    if bold {
        include_bytes!("../fonts/DejaVuSans-Bold.ttf")
    } else {
        include_bytes!("../fonts/DejaVuSans.ttf")
    }
}

fn paint_hex(hex: &str, opacity: f32) -> Paint {
    let (r, g, b, mut a) = parse_hex(hex).unwrap_or((0, 0, 0, 0));
    if hex == "none" {
        a = 0;
    }
    a = (a as f32 * opacity.clamp(0.0, 1.0)) as u8;
    let mut paint = Paint::default();
    paint.set_color_rgba8(r, g, b, a);
    paint.anti_alias = true;
    paint
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8, u8)> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    let a = if hex.len() == 8 {
        u8::from_str_radix(&hex[6..8], 16).ok()?
    } else {
        255
    };
    Some((r, g, b, a))
}

fn fill_if(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, paint: &Paint) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        pixmap.fill_rect(rect, paint, Transform::identity(), None);
    }
}

fn fill_round(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, radius: f32, paint: &Paint) {
    if let Some(path) = round_path(x, y, w, h, radius) {
        pixmap.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn stroke_round(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    width: f32,
    paint: &Paint,
) {
    if let Some(path) = round_path(x, y, w, h, radius) {
        let mut stroke = Stroke::default();
        stroke.width = width.max(1.0);
        pixmap.stroke_path(&path, paint, &stroke, Transform::identity(), None);
    }
}

fn round_path(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Option<tiny_skia::Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = radius.max(0.0).min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

fn fill_oval(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, paint: &Paint) {
    if let Some(path) = oval_path(x, y, w, h) {
        pixmap.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn stroke_oval(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, width: f32, paint: &Paint) {
    if let Some(path) = oval_path(x, y, w, h) {
        let mut stroke = Stroke::default();
        stroke.width = width.max(1.0);
        pixmap.stroke_path(&path, paint, &stroke, Transform::identity(), None);
    }
}

fn oval_path(x: f32, y: f32, w: f32, h: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    pb.push_oval(Rect::from_xywh(x, y, w, h)?);
    pb.finish()
}

fn stroke_line(
    pixmap: &mut Pixmap,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    control: Option<(f32, f32)>,
    width: f32,
    paint: &Paint,
    shadow: Option<Shade>,
    round_cap: bool,
) {
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    if let Some((cx, cy)) = control {
        pb.quad_to(cx, cy, x2, y2);
    } else {
        pb.line_to(x2, y2);
    }
    let Some(path) = pb.finish() else {
        return;
    };
    if let Some(shade) = shadow {
        if let Some(mut layer) = Pixmap::new(pixmap.width(), pixmap.height()) {
            let shadow_paint = paint_hex("#000000", shade.opacity);
            let mut wide = Stroke::default();
            wide.width = width.max(1.0) + shade.blur.max(0.0) * 0.55;
            wide.line_cap = LineCap::Round;
            layer.stroke_path(&path, &shadow_paint, &wide, Transform::from_translate(shade.x, shade.y), None);
            let mut erase = Paint::default();
            erase.set_color_rgba8(0, 0, 0, 255);
            erase.blend_mode = tiny_skia::BlendMode::DestinationOut;
            let mut cut = Stroke::default();
            cut.width = width.max(1.0);
            cut.line_cap = if round_cap { LineCap::Round } else { LineCap::Butt };
            layer.stroke_path(&path, &erase, &cut, Transform::identity(), None);
            pixmap.draw_pixmap(0, 0, layer.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
        }
    }
    let mut stroke = Stroke::default();
    stroke.width = width.max(1.0);
    stroke.line_cap = if round_cap { LineCap::Round } else { LineCap::Butt };
    pixmap.stroke_path(&path, paint, &stroke, Transform::identity(), None);
}

fn shadow_head(pixmap: &mut Pixmap, x: f32, y: f32, dx: f32, dy: f32, width: f32, shade: Option<Shade>) {
    let Some(shade) = shade else {
        return;
    };
    let paint = paint_hex("#000000", shade.opacity);
    draw_head(pixmap, x + shade.x, y + shade.y, dx, dy, width + shade.blur.max(0.0) * 0.35, &paint);
}

fn draw_head(pixmap: &mut Pixmap, x: f32, y: f32, dx: f32, dy: f32, width: f32, paint: &Paint) {
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let ux = dx / len;
    let uy = dy / len;
    let size = (width * 3.2).max(10.0);
    let bx = x - ux * size;
    let by = y - uy * size;
    let px = -uy * size * 0.45;
    let py = ux * size * 0.45;
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    pb.line_to(bx + px, by + py);
    pb.line_to(bx - px, by - py);
    pb.close();
    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
    }
}

struct Shade {
    blur: f32,
    x: f32,
    y: f32,
    opacity: f32,
}

fn shade(on: bool, blur: f32, x: f32, y: f32, opacity: f32) -> Option<Shade> {
    if on && opacity > 0.0 {
        Some(Shade { blur, x, y, opacity })
    } else {
        None
    }
}

fn pull_back(x: f32, y: f32, dx: f32, dy: f32, inset: f32) -> (f32, f32) {
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let cut = inset.min(len * 0.45);
    (x - dx / len * cut, y - dy / len * cut)
}

fn shadow_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, radius: f32, shade: Option<Shade>) {
    let Some(shade) = shade else {
        return;
    };
    blit_outer_shadow(pixmap, x, y, w, h, radius, shade.x, shade.y, shade.blur, shade.opacity, false);
}

fn shadow_oval(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, shade: Option<Shade>) {
    let Some(shade) = shade else {
        return;
    };
    blit_outer_shadow(pixmap, x, y, w, h, 0.0, shade.x, shade.y, shade.blur, shade.opacity, true);
}

fn blit_outer_shadow(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    ox: f32,
    oy: f32,
    blur: f32,
    opacity: f32,
    oval: bool,
) {
    if opacity <= 0.0 || w <= 0.0 || h <= 0.0 {
        return;
    }
    let Some(mut layer) = Pixmap::new(pixmap.width(), pixmap.height()) else {
        return;
    };
    let layers = 5;
    for i in 0..layers {
        let t = (i + 1) as f32 / layers as f32;
        let spread = blur.max(0.0) * t * 0.5;
        let alpha = opacity * (1.0 - t * 0.6) / layers as f32;
        let paint = paint_hex("#000000", alpha);
        let sx = x + ox - spread;
        let sy = y + oy - spread;
        let sw = w + spread * 2.0;
        let sh = h + spread * 2.0;
        if oval {
            fill_oval(&mut layer, sx, sy, sw, sh, &paint);
        } else {
            fill_round(&mut layer, sx, sy, sw, sh, radius + spread, &paint);
        }
    }
    let mut erase = Paint::default();
    erase.set_color_rgba8(0, 0, 0, 255);
    erase.blend_mode = tiny_skia::BlendMode::DestinationOut;
    erase.anti_alias = true;
    if oval {
        fill_oval(&mut layer, x, y, w, h, &erase);
    } else {
        fill_round(&mut layer, x, y, w, h, radius, &erase);
    }
    pixmap.draw_pixmap(0, 0, layer.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
}

fn image_to_pixmap(image: &RgbaImage) -> Pixmap {
    let mut pixmap = Pixmap::new(image.width(), image.height()).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
    let data = pixmap.data_mut();
    for (i, p) in image.pixels().enumerate() {
        let a = p.0[3] as u16;
        data[i * 4] = (p.0[0] as u16 * a / 255) as u8;
        data[i * 4 + 1] = (p.0[1] as u16 * a / 255) as u8;
        data[i * 4 + 2] = (p.0[2] as u16 * a / 255) as u8;
        data[i * 4 + 3] = p.0[3];
    }
    pixmap
}

fn pixmap_to_image(pixmap: &Pixmap) -> RgbaImage {
    let mut image = RgbaImage::new(pixmap.width(), pixmap.height());
    let data = pixmap.data();
    for (i, pixel) in image.pixels_mut().enumerate() {
        let a = data[i * 4 + 3] as u16;
        if a == 0 {
            *pixel = Rgba([0, 0, 0, 0]);
            continue;
        }
        pixel.0 = [
            (data[i * 4] as u16 * 255 / a).min(255) as u8,
            (data[i * 4 + 1] as u16 * 255 / a).min(255) as u8,
            (data[i * 4 + 2] as u16 * 255 / a).min(255) as u8,
            data[i * 4 + 3],
        ];
    }
    image
}

pub fn cursor_png() -> String {
    let mut image = RgbaImage::from_pixel(24, 24, Rgba([0, 0, 0, 0]));
    for y in 1..18 {
        image.put_pixel(2, y, Rgba([0, 0, 0, 255]));
        image.put_pixel(3, y, Rgba([255, 255, 255, 255]));
    }
    for i in 0..8 {
        image.put_pixel(3 + i, 17 - i / 2, Rgba([0, 0, 0, 255]));
        image.put_pixel(4 + i, 16 - i / 2, Rgba([255, 255, 255, 255]));
    }
    let mut bytes = Vec::new();
    let _ = image.write_with_encoder(image::codecs::png::PngEncoder::new(&mut bytes));
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[derive(Clone, Debug)]
pub struct BeautifyOptions {
    pub scale: f32,
    pub backdrop: String,
    pub padding: f32,
    pub radius: f32,
    pub shadow: bool,
    pub frame: bool,
    pub watermark: String,
}

pub fn beautify(image: &RgbaImage, options: &BeautifyOptions) -> RgbaImage {
    let scale = options.scale.clamp(0.25, 2.0);
    let pad = options.padding.clamp(0.0, 240.0);
    let radius = options.radius.clamp(0.0, 80.0);
    let scaled = if (scale - 1.0).abs() < 0.01 {
        image.clone()
    } else {
        let width = ((image.width() as f32) * scale).round().max(1.0) as u32;
        let height = ((image.height() as f32) * scale).round().max(1.0) as u32;
        image::imageops::resize(image, width, height, image::imageops::FilterType::Triangle)
    };
    let watermark = options.watermark.trim();
    let caption = if watermark.is_empty() { 0.0 } else { 32.0 };
    let width = (scaled.width() as f32 + pad * 2.0).round().max(1.0) as u32;
    let height = (scaled.height() as f32 + pad * 2.0 + caption).round().max(1.0) as u32;
    let mut pixmap = Pixmap::new(width, height).unwrap_or_else(|| Pixmap::new(1, 1).expect("pixmap"));
    let [r, g, b] = hex_rgb(&options.backdrop);
    pixmap.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));
    let x = pad;
    let y = pad;
    let w = scaled.width() as f32;
    let h = scaled.height() as f32;
    if options.shadow {
        blit_outer_shadow(&mut pixmap, x, y, w, h, radius, 0.0, 10.0, 18.0, 0.35, false);
    }
    let photo = image_to_pixmap(&scaled);
    pixmap.draw_pixmap(
        x.round() as i32,
        y.round() as i32,
        photo.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    if options.frame {
        let paint = paint_hex("#000000", 0.18);
        stroke_round(&mut pixmap, x, y, w, h, radius, 2.0, &paint);
    }
    if !watermark.is_empty() {
        draw_text(
            &mut pixmap,
            &shotsmith_core::TextObject {
                id: "watermark".into(),
                x: pad,
                y: pad + h + 6.0,
                text: watermark.to_string(),
                size: 14.0,
                color: "#3A3A3C".into(),
                fill: "none".into(),
                bold: false,
            },
        );
    }
    let mut image = pixmap_to_image(&pixmap);
    if radius > 0.0 {
        mask_radius(&mut image, x, y, w, h, radius, r, g, b);
    }
    image
}

fn hex_rgb(hex: &str) -> [u8; 3] {
    let hex = hex.trim().trim_start_matches('#');
    let Ok(value) = u32::from_str_radix(hex, 16) else {
        return [255, 255, 255];
    };
    if hex.len() == 6 {
        [(value >> 16) as u8, (value >> 8) as u8, value as u8]
    } else {
        [255, 255, 255]
    }
}

fn mask_radius(image: &mut RgbaImage, x: f32, y: f32, w: f32, h: f32, radius: f32, r: u8, g: u8, b: u8) {
    let radius = radius.min(w / 2.0).min(h / 2.0);
    let left = x.round() as i32;
    let top = y.round() as i32;
    let right = (x + w).round() as i32;
    let bottom = (y + h).round() as i32;
    for py in top..bottom {
        for px in left..right {
            if px < 0 || py < 0 || px as u32 >= image.width() || py as u32 >= image.height() {
                continue;
            }
            let dx = if (px as f32) < x + radius {
                x + radius - px as f32
            } else if (px as f32) > x + w - radius {
                px as f32 - (x + w - radius)
            } else {
                0.0
            };
            let dy = if (py as f32) < y + radius {
                y + radius - py as f32
            } else if (py as f32) > y + h - radius {
                py as f32 - (y + h - radius)
            } else {
                0.0
            };
            if dx * dx + dy * dy <= radius * radius {
                continue;
            }
            image.put_pixel(px as u32, py as u32, Rgba([r, g, b, 255]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shotsmith_core::{ObfuscateMode, ObfuscateObject};

    fn checker(size: u32) -> RgbaImage {
        let mut image = RgbaImage::new(size, size);
        for y in 0..size {
            for x in 0..size {
                let on = (x + y) % 2 == 0;
                let v = if on { 255 } else { 0 };
                image.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
        image
    }

    #[test]
    fn solid_redaction_removes_original_pixels() {
        let image = checker(16);
        let doc = Document {
            version: 1,
            width: 16,
            height: 16,
            objects: vec![AnnotationObject::Obfuscate(ObfuscateObject {
                id: "o".into(),
                x: 0.0,
                y: 0.0,
                w: 8.0,
                h: 8.0,
                mode: ObfuscateMode::Solid,
                block_size: 8,
                color: "#000000".into(),
            })],
        };
        let out = render(&image, &doc, &[]);
        assert_eq!(image.get_pixel(0, 0).0, [255, 255, 255, 255]);
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(out.get_pixel(x, y).0, [0, 0, 0, 255], "{x},{y}");
            }
        }
        assert_ne!(out.get_pixel(0, 0).0, image.get_pixel(0, 0).0);
        assert_eq!(out.get_pixel(15, 15).0, image.get_pixel(15, 15).0);
    }

    #[test]
    fn shadow_stays_outside_an_empty_rect() {
        use shotsmith_core::RectObject;
        let image = RgbaImage::from_pixel(40, 40, Rgba([255, 255, 255, 255]));
        let doc = Document {
            version: 1,
            width: 40,
            height: 40,
            objects: vec![AnnotationObject::Rect(RectObject {
                id: "r".into(),
                x: 12.0,
                y: 10.0,
                w: 16.0,
                h: 14.0,
                stroke: "#000000".into(),
                stroke_width: 2.0,
                fill: "none".into(),
                radius: 0.0,
                opacity: 1.0,
                shadow: true,
                shadow_blur: 6.0,
                shadow_x: 0.0,
                shadow_y: 4.0,
                shadow_opacity: 0.9,
            })],
        };
        let out = render(&image, &doc, &[]);
        let center = out.get_pixel(20, 17).0;
        assert!(center[0] > 200, "interior darkened: {center:?}");
        let below = out.get_pixel(20, 27).0;
        assert!(below[0] < 230, "no outer shadow: {below:?}");
    }

    #[test]
    fn drop_shadow_sits_outside_the_image() {
        let image = RgbaImage::from_pixel(20, 20, Rgba([240, 240, 240, 255]));
        let out = render(&image, &Document::new(20, 20), &[Effect::Shadow]);
        assert!(out.width() > 20, "shadow did not pad the canvas");
        let pad = (out.width() - 20) / 2;
        let inside = out.get_pixel(pad + 10, pad + 10).0;
        assert!(inside[0] > 220 && inside[3] == 255, "shadow filled the image: {inside:?}");
        let below = out.get_pixel(pad + 10, pad + 20 + 8).0;
        let above = out.get_pixel(pad + 10, 2).0;
        assert!(below[3] > above[3], "shadow falls inward: below {below:?} above {above:?}");
    }

    #[test]
    fn pixelate_does_not_keep_the_source_pattern() {
        let image = checker(16);
        let doc = Document {
            version: 1,
            width: 16,
            height: 16,
            objects: vec![AnnotationObject::Obfuscate(ObfuscateObject {
                id: "o".into(),
                x: 0.0,
                y: 0.0,
                w: 8.0,
                h: 8.0,
                mode: ObfuscateMode::Pixelate,
                block_size: 4,
                color: "#000000".into(),
            })],
        };
        let out = render(&image, &doc, &[]);
        let block = out.get_pixel(0, 0).0;
        assert_eq!(out.get_pixel(1, 0).0, block);
        assert_ne!(block, image.get_pixel(0, 0).0);
    }

    #[test]
    fn magnify_does_not_reveal_a_redaction() {
        let image = RgbaImage::from_pixel(32, 32, Rgba([255, 0, 0, 255]));
        let doc = Document {
            version: 1,
            width: 32,
            height: 32,
            objects: vec![
                AnnotationObject::Obfuscate(ObfuscateObject {
                    id: "o".into(),
                    x: 8.0,
                    y: 8.0,
                    w: 16.0,
                    h: 16.0,
                    mode: ObfuscateMode::Solid,
                    block_size: 8,
                    color: "#000000".into(),
                }),
                AnnotationObject::Magnify(MagnifyObject {
                    id: "m".into(),
                    x: 8.0,
                    y: 8.0,
                    w: 16.0,
                    h: 16.0,
                    factor: 2.0,
                }),
            ],
        };
        let out = render(&image, &doc, &[]);
        assert_eq!(out.get_pixel(16, 16).0, [0, 0, 0, 255]);
    }
}
