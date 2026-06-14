use std::path::PathBuf;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;
use crate::commands::IntoTauri;
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

impl FramePresetInput {
    fn into_preset(self, id: Uuid) -> FramePreset {
        FramePreset {
            id,
            name: self.name,
            landscape_frame_path: self.landscape_frame_path,
            portrait_frame_path: self.portrait_frame_path,
            target_ratio_w: self.target_ratio_w,
            target_ratio_h: self.target_ratio_h,
            crop_method: self.crop_method,
        }
    }

    fn apply(self, existing: &mut FramePreset) {
        let id = existing.id;
        *existing = self.into_preset(id);
    }
}

#[tauri::command]
pub async fn create_frame_preset(
    event_id: Uuid,
    preset: FramePresetInput,
    state: State<'_, AppState>,
) -> Result<FramePreset, String> {
    let mut event = state.store.load(event_id).tauri()?;
    let preset = preset.into_preset(Uuid::new_v4());
    event.frame_presets.push(preset.clone());
    state.store.save(&event).tauri()?;
    Ok(preset)
}

#[tauri::command]
pub async fn update_frame_preset(
    event_id: Uuid,
    preset_id: Uuid,
    preset: FramePresetInput,
    state: State<'_, AppState>,
) -> Result<FramePreset, String> {
    let mut event = state.store.load(event_id).tauri()?;
    let existing = event.find_frame_preset_mut(preset_id)?;
    preset.apply(existing);
    let updated = existing.clone();
    state.store.save(&event).tauri()?;
    state.invalidate_preview_for_preset(preset_id);
    Ok(updated)
}

#[tauri::command]
pub async fn delete_frame_preset(
    event_id: Uuid,
    preset_id: Uuid,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut event = state.store.load(event_id).tauri()?;
    event.frame_presets.retain(|p| p.id != preset_id);
    if event.active_frame_preset_id == Some(preset_id) {
        event.active_frame_preset_id = event.frame_presets.first().map(|p| p.id);
    }
    state.store.save(&event).tauri()?;
    state.invalidate_preview_for_preset(preset_id);
    Ok(())
}
