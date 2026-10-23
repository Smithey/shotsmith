use image::{Rgba, RgbaImage};
use shotsmith_platform::{FrozenDisplay, PlatformResult};

#[derive(Clone, Copy, Debug)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn crop(image: &RgbaImage, rect: PixelRect) -> RgbaImage {
    let x = rect.x.max(0) as u32;
    let y = rect.y.max(0) as u32;
    if x >= image.width() || y >= image.height() {
        return RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
    }
    let width = rect.width.max(1).min(image.width() - x);
    let height = rect.height.max(1).min(image.height() - y);
    image::imageops::crop_imm(image, x, y, width, height).to_image()
}

pub fn display_at(displays: &[FrozenDisplay], x: i32, y: i32) -> Option<&FrozenDisplay> {
    displays.iter().find(|display| {
        let info = &display.info;
        x >= info.x
            && y >= info.y
            && x < info.x + info.width as i32
            && y < info.y + info.height as i32
    })
}

pub fn freeze() -> PlatformResult<Vec<FrozenDisplay>> {
    shotsmith_platform::freeze_all()
}

pub fn pages_same(a: &RgbaImage, b: &RgbaImage) -> bool {
    if a.width() != b.width() || a.height() != b.height() {
        return false;
    }
    sample_diff(a, 0, b, 0, a.height()) < 12
}

pub fn stitch_vertical(pages: &[RgbaImage]) -> RgbaImage {
    let Some(first) = pages.first() else {
        return RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
    };
    let mut combined = first.clone();
    for page in pages.iter().skip(1) {
        if page.width() != combined.width() || page.height() < 8 {
            continue;
        }
        if pages_same(&crop_bottom(&combined, page.height().min(combined.height())), page) {
            break;
        }
        let overlap = best_overlap(&combined, page);
        let extra = page.height().saturating_sub(overlap);
        if extra == 0 {
            break;
        }
        let mut next = RgbaImage::new(combined.width(), combined.height() + extra);
        for (x, y, pixel) in combined.enumerate_pixels() {
            next.put_pixel(x, y, *pixel);
        }
        for y in 0..extra {
            for x in 0..page.width() {
                next.put_pixel(x, combined.height() + y, *page.get_pixel(x, overlap + y));
            }
        }
        combined = next;
    }
    combined
}

fn crop_bottom(image: &RgbaImage, height: u32) -> RgbaImage {
    let height = height.min(image.height());
    let y = image.height() - height;
    image::imageops::crop_imm(image, 0, y, image.width(), height).to_image()
}

fn best_overlap(base: &RgbaImage, next: &RgbaImage) -> u32 {
    let limit = base.height().min(next.height());
    let start = (limit / 5).max(4);
    let end = (limit * 4 / 5).max(start);
    let mut best = start;
    let mut best_score = u64::MAX;
    let mut overlap = start;
    while overlap <= end {
        let score = sample_diff(base, base.height() - overlap, next, 0, overlap);
        if score < best_score {
            best_score = score;
            best = overlap;
        }
        overlap += 4;
    }
    if best_score > 48 {
        0
    } else {
        best
    }
}

fn sample_diff(a: &RgbaImage, ay: u32, b: &RgbaImage, by: u32, rows: u32) -> u64 {
    if rows == 0 || a.width() == 0 {
        return u64::MAX;
    }
    let mut total = 0u64;
    let mut count = 0u64;
    let mut row = 0;
    while row < rows {
        let mut x = 0;
        while x < a.width().min(b.width()) {
            let pa = a.get_pixel(x, ay + row).0;
            let pb = b.get_pixel(x, by + row).0;
            total += (pa[0] as i16 - pb[0] as i16).unsigned_abs() as u64;
            total += (pa[1] as i16 - pb[1] as i16).unsigned_abs() as u64;
            total += (pa[2] as i16 - pb[2] as i16).unsigned_abs() as u64;
            count += 3;
            x += 8;
        }
        row += 2;
    }
    if count == 0 { u64::MAX } else { total / count }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_stays_inside_the_image() {
        let mut image = RgbaImage::new(10, 8);
        image.put_pixel(1, 1, Rgba([9, 8, 7, 255]));
        let cropped = crop(
            &image,
            PixelRect {
                x: 1,
                y: 1,
                width: 50,
                height: 50,
            },
        );
        assert_eq!(cropped.width(), 9);
        assert_eq!(cropped.height(), 7);
        assert_eq!(cropped.get_pixel(0, 0).0, [9, 8, 7, 255]);
    }

    #[test]
    fn stitch_drops_the_overlapping_rows() {
        let mut first = RgbaImage::from_pixel(8, 20, Rgba([10, 10, 10, 255]));
        let mut second = RgbaImage::from_pixel(8, 20, Rgba([200, 200, 200, 255]));
        for y in 12..20 {
            for x in 0..8 {
                let pixel = Rgba([40, 40, y as u8, 255]);
                first.put_pixel(x, y, pixel);
                second.put_pixel(x, y - 12, pixel);
            }
        }
        let stitched = stitch_vertical(&[first, second]);
        assert!(stitched.height() > 20);
        assert!(stitched.height() < 40);
    }
}
