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

    fn cache_key(photo_path: &Path) -> String {
        let meta = std::fs::metadata(photo_path).ok();
        let mtime = meta
            .and_then(|m| m.modified().ok())
            .map(|t| format!("{:?}", t))
            .unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(photo_path.to_string_lossy().as_bytes());
        hasher.update(mtime.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn thumb_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.jpg", key))
    }

    pub fn get_or_generate(&self, photo_path: &Path) -> Result<Vec<u8>> {
        let key = Self::cache_key(photo_path);
        let path = self.thumb_path(&key);
        if path.exists() {
            return std::fs::read(&path).context("reading cached thumbnail");
        }
        self.generate(photo_path, &path)
    }

    fn generate(&self, photo_path: &Path, out_path: &Path) -> Result<Vec<u8>> {
        let img = image::open(photo_path)
            .with_context(|| format!("opening {} for thumbnail", photo_path.display()))?;
        let thumb = img.thumbnail(THUMB_SIZE, THUMB_SIZE);
        thumb
            .save_with_format(out_path, image::ImageFormat::Jpeg)
            .context("saving thumbnail")?;
        std::fs::read(out_path).context("reading generated thumbnail")
    }

    pub fn invalidate(&self, photo_path: &Path) {
        let key = Self::cache_key(photo_path);
        let _ = std::fs::remove_file(self.thumb_path(&key));
    }
}
