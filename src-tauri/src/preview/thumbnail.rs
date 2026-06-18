use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use sha2::{Sha256, Digest};

const THUMB_SIZE: u32 = 256;

pub struct ThumbnailCache {
    cache_dir: PathBuf,
}

impl ThumbnailCache {
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    /// Cache key. Prefer the caller's content hash (busts on any byte change —
    /// EXIF-only rotations included, even when the editor preserves mtime).
    /// Falls back to path + mtime when no hash is supplied.
    fn cache_key(photo_path: &Path, content_hash: Option<&str>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(photo_path.to_string_lossy().as_bytes());
        match content_hash {
            Some(h) => hasher.update(h.as_bytes()),
            None => {
                let mtime = std::fs::metadata(photo_path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| format!("{:?}", t))
                    .unwrap_or_default();
                hasher.update(mtime.as_bytes());
            }
        }
        format!("{:x}", hasher.finalize())
    }

    fn thumb_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.jpg", key))
    }

    pub fn get_or_generate(&self, photo_path: &Path, content_hash: Option<&str>) -> Result<Vec<u8>> {
        let key = Self::cache_key(photo_path, content_hash);
        let path = self.thumb_path(&key);
        if path.exists() {
            return std::fs::read(&path).context("reading cached thumbnail");
        }
        self.generate(photo_path, &path)
    }

    fn generate(&self, photo_path: &Path, out_path: &Path) -> Result<Vec<u8>> {
        // Route through load_photo so EXIF orientation is applied (upright thumb).
        let img = crate::photo::loader::load_photo(photo_path)
            .with_context(|| format!("opening {} for thumbnail", photo_path.display()))?;
        let thumb = img.thumbnail(THUMB_SIZE, THUMB_SIZE);
        let mut buf = Vec::new();
        thumb
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg)
            .context("encoding thumbnail")?;
        std::fs::write(out_path, &buf).context("saving thumbnail")?;
        Ok(buf)
    }

}
