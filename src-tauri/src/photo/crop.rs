use image::DynamicImage;
use crate::project::model::{CropMethod, CropRect};

/// Compute the crop rect that fits `target_ratio` (w/h) inside `(img_w, img_h)`.
pub fn compute_crop_rect(
    img_w: u32,
    img_h: u32,
    target_ratio: f32,
    method: CropMethod,
) -> CropRect {
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

    let (x, y) = match method {
        CropMethod::Center => {
            ((img_w - crop_w) / 2, (img_h - crop_h) / 2)
        }
        CropMethod::RuleOfThirds => {
            // Bias the crop toward a rule-of-thirds line rather than dead center,
            // along whichever axis actually has slack to move.
            if img_ratio > target_ratio {
                // Horizontal slack: center the subject on the left third line.
                let slack = img_w - crop_w;
                let x = (img_w / 3).saturating_sub(crop_w / 2).min(slack);
                let y = (img_h - crop_h) / 2;
                (x, y)
            } else {
                // Vertical slack: center the subject on the upper third line.
                let slack = img_h - crop_h;
                let x = (img_w - crop_w) / 2;
                let y = (img_h / 3).saturating_sub(crop_h / 2).min(slack);
                (x, y)
            }
        }
    };

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
        let r = compute_crop_rect(100, 100, 1.0, CropMethod::Center);
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 100, 100));
    }

    #[test]
    fn center_crop_wide_image_trims_width_and_centers() {
        // 200x100 to 1:1 → 100x100 crop centered horizontally.
        let r = compute_crop_rect(200, 100, 1.0, CropMethod::Center);
        assert_eq!((r.width, r.height), (100, 100));
        assert_eq!((r.x, r.y), (50, 0));
    }

    #[test]
    fn center_crop_tall_image_trims_height_and_centers() {
        // 100x200 to 1:1 → 100x100 crop centered vertically.
        let r = compute_crop_rect(100, 200, 1.0, CropMethod::Center);
        assert_eq!((r.width, r.height), (100, 100));
        assert_eq!((r.x, r.y), (0, 50));
    }

    #[test]
    fn rule_of_thirds_wide_biases_to_left_third() {
        // 300x100 to 1:1 → 100x100 crop, slack=200, x = 100 - 50 = 50.
        let r = compute_crop_rect(300, 100, 1.0, CropMethod::RuleOfThirds);
        assert_eq!((r.width, r.height), (100, 100));
        assert_eq!(r.x, 50);
        assert_eq!(r.y, 0);
        // Differs from center (which would be x=100).
        let c = compute_crop_rect(300, 100, 1.0, CropMethod::Center);
        assert_ne!(r.x, c.x);
    }

    #[test]
    fn rule_of_thirds_tall_biases_to_upper_third() {
        // 100x300 to 1:1 → 100x100 crop, slack=200, y = 100 - 50 = 50.
        let r = compute_crop_rect(100, 300, 1.0, CropMethod::RuleOfThirds);
        assert_eq!((r.width, r.height), (100, 100));
        assert_eq!(r.y, 50);
        assert_eq!(r.x, 0);
    }

    #[test]
    fn crop_rect_stays_within_image_bounds() {
        let r = compute_crop_rect(640, 480, 16.0 / 9.0, CropMethod::RuleOfThirds);
        assert!(r.x + r.width <= 640);
        assert!(r.y + r.height <= 480);
    }

    #[test]
    fn apply_crop_produces_expected_dimensions() {
        let img = DynamicImage::ImageRgb8(RgbImage::new(200, 100));
        let r = compute_crop_rect(200, 100, 1.0, CropMethod::Center);
        let out = apply_crop(&img, r);
        assert_eq!((out.width(), out.height()), (100, 100));
    }
}
