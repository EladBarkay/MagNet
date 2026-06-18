use std::path::Path;
use anyhow::{Context, Result};
use image::DynamicImage;

const PRINT_DPI: u16 = 300;

/// Write a framed image as RGB JPEG with 300 DPI metadata.
pub fn write_print_ready(image: &DynamicImage, output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Avoid a full-canvas copy when the compositor already produced RGB8.
    let rgb_owned;
    let rgb = match image.as_rgb8() {
        Some(b) => b,
        None => {
            rgb_owned = image.to_rgb8();
            &rgb_owned
        }
    };
    let mut buf = Vec::new();
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::Cursor::new(&mut buf), 95);
    encoder
        .encode_image(rgb)
        .with_context(|| format!("encoding JPEG for {}", output_path.display()))?;

    set_jfif_dpi(&mut buf, PRINT_DPI);

    std::fs::write(output_path, &buf)
        .with_context(|| format!("writing {}", output_path.display()))
}

/// Patch the JFIF APP0 density fields to `dpi` (units = dots/inch).
/// The `image` crate emits a standard APP0 segment immediately after the SOI
/// marker; we rewrite the units + X/Y density in place. No-op if the layout
/// isn't the expected JFIF APP0.
fn set_jfif_dpi(jpeg: &mut [u8], dpi: u16) {
    // SOI (FFD8) then APP0 (FFE0) with "JFIF\0" identifier.
    if jpeg.len() < 18 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 || jpeg[2] != 0xFF || jpeg[3] != 0xE0 {
        return;
    }
    if &jpeg[6..11] != b"JFIF\0" {
        return;
    }
    // Layout from APP0 marker: FFE0 len(2) "JFIF\0"(5) version(2) units(1) Xd(2) Yd(2)
    // Absolute offsets: units=13, Xdensity=14..16, Ydensity=16..18.
    jpeg[13] = 1; // units: dots per inch
    jpeg[14..16].copy_from_slice(&dpi.to_be_bytes());
    jpeg[16..18].copy_from_slice(&dpi.to_be_bytes());
}
