use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub name: String,
    /// The folder used to open/identify this event session (not necessarily a batch folder).
    #[serde(default)]
    pub root_path: Option<PathBuf>,
    pub batches: Vec<PhotoBatch>,
    pub frame_presets: Vec<FramePreset>,
    pub canvas_presets: Vec<CanvasPreset>,
    pub output_folder: Option<PathBuf>,
    pub active_frame_preset_id: Option<Uuid>,
    /// Last-used canvas preset; restored on reload (default keeps old magnet.json valid).
    #[serde(default)]
    pub active_canvas_preset_id: Option<Uuid>,
}

impl Event {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            root_path: None,
            batches: Vec::new(),
            frame_presets: Vec::new(),
            canvas_presets: Vec::new(),
            output_folder: None,
            active_frame_preset_id: None,
            active_canvas_preset_id: None,
        }
    }

    /// Find a canvas preset by id, or a "not found" error message suitable for IPC.
    pub fn find_canvas_preset(&self, id: Uuid) -> Result<&CanvasPreset, String> {
        self.canvas_presets
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("canvas preset {id} not found"))
    }

    /// Find a canvas preset by id (mutable).
    pub fn find_canvas_preset_mut(&mut self, id: Uuid) -> Result<&mut CanvasPreset, String> {
        self.canvas_presets
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("canvas preset {id} not found"))
    }

    /// Find a frame preset by id, or a "not found" error message suitable for IPC.
    pub fn find_frame_preset(&self, id: Uuid) -> Result<&FramePreset, String> {
        self.frame_presets
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("frame preset {id} not found"))
    }

    /// Find a frame preset by id (mutable).
    pub fn find_frame_preset_mut(&mut self, id: Uuid) -> Result<&mut FramePreset, String> {
        self.frame_presets
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("frame preset {id} not found"))
    }

    /// Find a photo by id across all batches.
    pub fn find_photo(&self, id: Uuid) -> Result<&Photo, String> {
        self.batches
            .iter()
            .flat_map(|b| &b.photos)
            .find(|p| p.id == id)
            .ok_or_else(|| format!("photo {id} not found"))
    }

    /// Find a photo by id across all batches (mutable).
    pub fn find_photo_mut(&mut self, id: Uuid) -> Result<&mut Photo, String> {
        self.batches
            .iter_mut()
            .flat_map(|b| &mut b.photos)
            .find(|p| p.id == id)
            .ok_or_else(|| format!("photo {id} not found"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoBatch {
    pub id: Uuid,
    pub name: String,
    pub source_path: PathBuf,
    pub photos: Vec<Photo>,
}

impl PhotoBatch {
    pub fn new(name: String, source_path: PathBuf) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            source_path,
            photos: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    pub id: Uuid,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub exif_orientation: Option<Orientation>,
    pub orientation_override: Option<Orientation>,
    pub crop_override: Option<CropRect>,
    pub print_count: u32,
    #[serde(default)]
    pub save_count: u32,
    pub content_hash: String,
    // Queued copies for the next export/print run, persisted so a closed+reopened
    // event restores each card's last value. Defaults to 1 (old events / new photos).
    #[serde(default = "default_copies")]
    pub copies: u32,
    // File metadata for gallery sorting. Epoch seconds; 0 when unknown (e.g. old
    // events or platforms without created-time). Derived from the file, not user data.
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub modified: u64,
}

fn default_copies() -> u32 {
    1
}

impl Photo {
    pub fn effective_orientation(&self) -> Orientation {
        self.orientation_override
            .unwrap_or(
                if self.width >= self.height {
                    Orientation::Landscape
                } else {
                    Orientation::Portrait
                }
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Landscape,
    Portrait,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FramePreset {
    pub id: Uuid,
    pub name: String,
    pub landscape_frame_path: PathBuf,
    pub portrait_frame_path: PathBuf,
    /// Target ratio as (width, height), e.g. (4.0, 3.0)
    pub target_ratio_w: f32,
    pub target_ratio_h: f32,
}

impl FramePreset {
    pub fn frame_path(&self, orientation: Orientation) -> &PathBuf {
        match orientation {
            Orientation::Landscape => &self.landscape_frame_path,
            Orientation::Portrait => &self.portrait_frame_path,
        }
    }

    pub fn target_ratio(&self) -> f32 {
        self.target_ratio_w / self.target_ratio_h
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasPreset {
    pub id: Uuid,
    pub name: String,
    pub canvas_width_px: u32,
    pub canvas_height_px: u32,
    pub photos_per_canvas: u8,
    pub dpi: u32,
    pub cols: u8,
    pub rows: u8,
}

impl CanvasPreset {
    pub fn slot_width(&self) -> u32 {
        self.canvas_width_px / self.cols as u32
    }

    pub fn slot_height(&self) -> u32 {
        self.canvas_height_px / self.rows as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo(width: u32, height: u32) -> Photo {
        Photo {
            id: Uuid::new_v4(),
            path: PathBuf::from("x.jpg"),
            width,
            height,
            exif_orientation: None,
            orientation_override: None,
            crop_override: None,
            print_count: 0,
            save_count: 0,
            content_hash: String::new(),
            copies: 1,
            size_bytes: 0,
            created: 0,
            modified: 0,
        }
    }

    #[test]
    fn orientation_from_pixels_landscape() {
        assert_eq!(photo(200, 100).effective_orientation(), Orientation::Landscape);
    }

    #[test]
    fn orientation_from_pixels_portrait() {
        assert_eq!(photo(100, 200).effective_orientation(), Orientation::Portrait);
    }

    #[test]
    fn square_is_landscape() {
        assert_eq!(photo(100, 100).effective_orientation(), Orientation::Landscape);
    }

    #[test]
    fn override_wins_over_pixels() {
        let mut p = photo(200, 100); // would be Landscape
        p.orientation_override = Some(Orientation::Portrait);
        assert_eq!(p.effective_orientation(), Orientation::Portrait);
    }

    #[test]
    fn slot_dimensions_divide_canvas_by_grid() {
        let preset = CanvasPreset {
            id: Uuid::new_v4(),
            name: "2-up".into(),
            canvas_width_px: 2400,
            canvas_height_px: 1600,
            photos_per_canvas: 2,
            dpi: 300,
            cols: 2,
            rows: 1,
        };
        assert_eq!(preset.slot_width(), 1200);
        assert_eq!(preset.slot_height(), 1600);
    }
}
