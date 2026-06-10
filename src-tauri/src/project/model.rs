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
        }
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
    pub xmp_path: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
    pub exif_orientation: Option<Orientation>,
    pub orientation_override: Option<Orientation>,
    pub crop_override: Option<CropRect>,
    pub print_count: u32,
    pub content_hash: String,
}

impl Photo {
    pub fn effective_orientation(&self) -> Orientation {
        self.orientation_override
            .unwrap_or_else(|| {
                if self.width >= self.height {
                    Orientation::Landscape
                } else {
                    Orientation::Portrait
                }
            })
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
    pub crop_method: CropMethod,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CropMethod {
    Center,
    RuleOfThirds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasPreset {
    pub id: Uuid,
    pub name: String,
    pub canvas_width_px: u32,
    pub canvas_height_px: u32,
    pub photos_per_canvas: u8,
    pub dpi: u32,
    pub margin_px: u32,
    pub cols: u8,
    pub rows: u8,
}

impl CanvasPreset {
    pub fn slot_width(&self) -> u32 {
        let total_margin = self.margin_px * (self.cols as u32 + 1);
        (self.canvas_width_px - total_margin) / self.cols as u32
    }

    pub fn slot_height(&self) -> u32 {
        let total_margin = self.margin_px * (self.rows as u32 + 1);
        (self.canvas_height_px - total_margin) / self.rows as u32
    }
}
