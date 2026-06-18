use image::DynamicImage;
use crate::project::model::CropRect;

/// Compute the centered crop rect that fits `target_ratio` (w/h) inside `(img_w, img_h)`.
pub fn compute_crop_rect(img_w: u32, img_h: u32, target_ratio: f32) -> CropRect {
    let img_ratio = img_w as f32 / img_h as f32;

    let (crop_w, crop_h) = if img_ratio > target_ratio {
        // Image is wider than target: crop width
        let h = img_h;
        let w = (h as f32 * target_ratio).round() as u32;
        (w, h)
    } else {
        // Image is taller than target: crop height
        let w = img_w;
        let h = (w as f32 / target_ratio).round() as u32;
        (w, h)
    };

    let x = (img_w - crop_w) / 2;
    let y = (img_h - crop_h) / 2;
    CropRect { x, y, width: crop_w, height: crop_h }
}

pub fn apply_crop(image: &DynamicImage, rect: CropRect) -> DynamicImage {
    image.crop_imm(rect.x, rect.y, rect.width, rect.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbImage};

    #[test]
    fn center_crop_square_to_square_is_full_image() {
        let r = compute_crop_rect(100, 100, 1.0);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 100, 100));
    }

    #[test]
    fn center_crop_wide_image_trims_width_and_centers() {
        // 200x100 to 1:1 → 100x100 crop centered horizontally.
        let r = compute_crop_rect(200, 100, 1.0);
        assert_eq!((r.width, r.height), (100, 100));
        assert_eq!((r.x, r.y), (50, 0));
    }

    #[test]
    fn center_crop_tall_image_trims_height_and_centers() {
        // 100x200 to 1:1 → 100x100 crop centered vertically.
        let r = compute_crop_rect(100, 200, 1.0);
        assert_eq!((r.width, r.height), (100, 100));
        assert_eq!((r.x, r.y), (0, 50));
    }

    #[test]
    fn crop_rect_stays_within_image_bounds() {
        let r = compute_crop_rect(640, 480, 16.0 / 9.0);
        assert!(r.x + r.width <= 640);
        assert!(r.y + r.height <= 480);
    }

    #[test]
    fn apply_crop_produces_expected_dimensions() {
        let img = DynamicImage::ImageRgb8(RgbImage::new(200, 100));
        let r = compute_crop_rect(200, 100, 1.0);
        let out = apply_crop(&img, r);
        assert_eq!((out.width(), out.height()), (100, 100));
    }
}
