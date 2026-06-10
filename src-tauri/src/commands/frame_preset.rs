use std::path::PathBuf;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;
use crate::project::model::{CropMethod, FramePreset};
use crate::AppState;

#[derive(Deserialize)]
pub struct FramePresetInput {
    pub name: String,
    pub landscape_frame_path: PathBuf,
    pub portrait_frame_path: PathBuf,
    pub target_ratio_w: f32,
    pub target_ratio_h: f32,
    pub crop_method: CropMethod,
}

#[tauri::command]
pub async fn list_frame_presets(
    event_id: Uuid,
    state: State<'_, AppState>,
) -> Result<Vec<FramePreset>, String> {
    let event = state.store.load(event_id).map_err(|e| e.to_string())?;
    Ok(event.frame_presets)
}

#[tauri::command]
pub async fn create_frame_preset(
    event_id: Uuid,
    preset: FramePresetInput,
    state: State<'_, AppState>,
) -> Result<FramePreset, String> {
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    let preset = FramePreset {
        id: Uuid::new_v4(),
        name: preset.name,
        landscape_frame_path: preset.landscape_frame_path,
        portrait_frame_path: preset.portrait_frame_path,
        target_ratio_w: preset.target_ratio_w,
        target_ratio_h: preset.target_ratio_h,
        crop_method: preset.crop_method,
    };
    event.frame_presets.push(preset.clone());
    state.store.save(&event).map_err(|e| e.to_string())?;
    Ok(preset)
}

#[tauri::command]
pub async fn delete_frame_preset(
    event_id: Uuid,
    preset_id: Uuid,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    event.frame_presets.retain(|p| p.id != preset_id);
    if event.active_frame_preset_id == Some(preset_id) {
        event.active_frame_preset_id = event.frame_presets.first().map(|p| p.id);
    }
    state.store.save(&event).map_err(|e| e.to_string())
}
