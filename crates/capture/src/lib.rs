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
}
