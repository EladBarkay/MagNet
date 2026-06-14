use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use image::DynamicImage;
use sha2::{Sha256, Digest};
use crate::project::model::{Orientation, Photo};
use uuid::Uuid;

pub struct LoadedPhoto {
    pub image: DynamicImage,
}

pub fn load_photo(path: &Path) -> Result<LoadedPhoto> {
    let image = image::open(path)
        .with_context(|| format!("opening image {}", path.display()))?;
    Ok(LoadedPhoto { image })
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
    if xmp.exists() { Some(xmp) } else { None }
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
    let reader = image::ImageReader::open(&path)
        .with_context(|| format!("scanning {}", path.display()))?
        .with_guessed_format()?;
    let (width, height) = reader.into_dimensions()
        .with_context(|| format!("reading dimensions of {}", path.display()))?;
    let xmp_path = xmp_path_for(&path);
    let content_hash = compute_content_hash(&path, xmp_path.as_deref())?;
    let exif_orientation = read_exif_orientation(&path);
    Ok(Photo {
        id: Uuid::new_v4(),
        path,
        width,
        height,
        exif_orientation,
        orientation_override: None,
        crop_override: None,
        print_count: 0,
        export_count: 0,
        content_hash,
    })
}

pub fn is_supported_image(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("jpg") | Some("jpeg") | Some("png") | Some("tif") | Some("tiff")
    )
}
