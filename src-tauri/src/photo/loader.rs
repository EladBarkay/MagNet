use crate::project::model::{Orientation, Photo};
use anyhow::{Context, Result};
use image::metadata::Orientation as ExifOrientation;
use image::{DynamicImage, ImageDecoder};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Decode an image with its EXIF orientation applied, so a photo rotated via
/// Explorer/Photos (which often only flips the EXIF Orientation tag) shows
/// upright everywhere. `image::open` ignores orientation, so we go through the
/// decoder explicitly.
pub fn load_photo(path: &Path) -> Result<DynamicImage> {
    let mut decoder = image::ImageReader::open(path)
        .with_context(|| format!("opening image {}", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("reading format of {}", path.display()))?
        .into_decoder()
        .with_context(|| format!("decoding {}", path.display()))?;
    let orientation = decoder
        .orientation()
        .unwrap_or(ExifOrientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder)
        .with_context(|| format!("decoding {}", path.display()))?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// Pixel dimensions after the EXIF orientation is applied — width/height swap on
/// a 90°/270° rotation. Keeps `Photo`'s stored dims (and thus
/// `effective_orientation`) consistent with the upright image.
fn upright_dims(w: u32, h: u32, orientation: ExifOrientation) -> (u32, u32) {
    match orientation {
        ExifOrientation::Rotate90
        | ExifOrientation::Rotate270
        | ExifOrientation::Rotate90FlipH
        | ExifOrientation::Rotate270FlipH => (h, w),
        _ => (w, h),
    }
}

pub fn read_exif_orientation(path: &Path) -> Option<Orientation> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;
    let field = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    match field.value.get_uint(0)? {
        // EXIF orientation values 1,3,6,8 indicate landscape or portrait after rotation
        1 | 3 => Some(Orientation::Landscape),
        6 | 8 => Some(Orientation::Portrait),
        _ => None,
    }
}

pub fn xmp_path_for(photo_path: &Path) -> Option<PathBuf> {
    let xmp = photo_path.with_extension("xmp");
    if xmp.exists() {
        Some(xmp)
    } else {
        None
    }
}

/// Content-based SHA-256 of the photo bytes followed by the XMP sidecar bytes
/// (per spec). Streams both files in chunks so memory stays bounded regardless
/// of file size. Changing either file changes the hash, which resets print_count
/// downstream in `merge_photos`.
pub fn compute_content_hash(photo_path: &Path, xmp_path: Option<&Path>) -> Result<String> {
    let mut hasher = Sha256::new();

    let mut file = std::fs::File::open(photo_path)
        .with_context(|| format!("opening {} for hashing", photo_path.display()))?;
    std::io::copy(&mut file, &mut hasher)
        .with_context(|| format!("hashing {}", photo_path.display()))?;

    if let Some(xmp) = xmp_path {
        if let Ok(mut xmp_file) = std::fs::File::open(xmp) {
            // Domain separator so photo+xmp can't collide with a single blob.
            hasher.update(b"\x00xmp\x00");
            let _ = std::io::copy(&mut xmp_file, &mut hasher);
        }
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Build a Photo record from a file path (dimensions + hash + orientation).
/// Does NOT load the full image into memory.
pub fn scan_photo(path: PathBuf) -> Result<Photo> {
    let mut decoder = image::ImageReader::open(&path)
        .with_context(|| format!("scanning {}", path.display()))?
        .with_guessed_format()?
        .into_decoder()
        .with_context(|| format!("reading dimensions of {}", path.display()))?;
    let (w0, h0) = decoder.dimensions();
    let orientation = decoder
        .orientation()
        .unwrap_or(ExifOrientation::NoTransforms);
    let (width, height) = upright_dims(w0, h0, orientation);
    let xmp_path = xmp_path_for(&path);
    let content_hash = compute_content_hash(&path, xmp_path.as_deref())?;
    let exif_orientation = read_exif_orientation(&path);
    let (size_bytes, created, modified) = file_times(&path);
    Ok(Photo {
        path,
        width,
        height,
        exif_orientation,
        orientation_override: None,
        crop_override: None,
        print_count: 0,
        save_count: 0,
        content_hash,
        copies: 1,
        size_bytes,
        created,
        modified,
    })
}

/// File size + created/modified as epoch seconds for gallery sorting. Best-effort:
/// any unavailable value (platform/fs limitation) falls back to 0.
fn file_times(path: &Path) -> (u64, u64, u64) {
    use std::time::UNIX_EPOCH;
    let Ok(meta) = std::fs::metadata(path) else {
        return (0, 0, 0);
    };
    let secs = |t: std::io::Result<std::time::SystemTime>| {
        t.ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0)
    };
    (meta.len(), secs(meta.created()), secs(meta.modified()))
}

pub fn is_supported_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("jpg") | Some("jpeg") | Some("png") | Some("tif") | Some("tiff")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upright_dims_swaps_on_quarter_turns_only() {
        // 90°/270° rotations swap; everything else keeps order.
        assert_eq!(
            upright_dims(6000, 4000, ExifOrientation::NoTransforms),
            (6000, 4000)
        );
        assert_eq!(
            upright_dims(6000, 4000, ExifOrientation::Rotate180),
            (6000, 4000)
        );
        assert_eq!(
            upright_dims(6000, 4000, ExifOrientation::FlipHorizontal),
            (6000, 4000)
        );
        assert_eq!(
            upright_dims(6000, 4000, ExifOrientation::Rotate90),
            (4000, 6000)
        );
        assert_eq!(
            upright_dims(6000, 4000, ExifOrientation::Rotate270),
            (4000, 6000)
        );
        assert_eq!(
            upright_dims(6000, 4000, ExifOrientation::Rotate90FlipH),
            (4000, 6000)
        );
        assert_eq!(
            upright_dims(6000, 4000, ExifOrientation::Rotate270FlipH),
            (4000, 6000)
        );
    }
}
