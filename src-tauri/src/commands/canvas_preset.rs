use serde::Deserialize;
use tauri::State;
use uuid::Uuid;
use crate::project::model::CanvasPreset;
use crate::AppState;

#[derive(Deserialize)]
pub struct CanvasPresetInput {
    pub name: String,
    pub canvas_width_px: u32,
    pub canvas_height_px: u32,
    pub photos_per_canvas: u8,
    pub dpi: u32,
    pub margin_px: u32,
    pub cols: u8,
    pub rows: u8,
}

impl CanvasPresetInput {
    fn into_preset(self, id: Uuid) -> CanvasPreset {
        CanvasPreset {
            id,
            name: self.name,
            canvas_width_px: self.canvas_width_px,
            canvas_height_px: self.canvas_height_px,
            photos_per_canvas: self.photos_per_canvas,
            dpi: self.dpi,
            margin_px: self.margin_px,
            cols: self.cols,
            rows: self.rows,
        }
    }

    fn apply(self, existing: &mut CanvasPreset) {
        let id = existing.id;
        *existing = self.into_preset(id);
    }
}

#[tauri::command]
pub async fn create_canvas_preset(
    event_id: Uuid,
    preset: CanvasPresetInput,
    state: State<'_, AppState>,
) -> Result<CanvasPreset, String> {
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    let preset = preset.into_preset(Uuid::new_v4());
    event.canvas_presets.push(preset.clone());
    state.store.save(&event).map_err(|e| e.to_string())?;
    Ok(preset)
}

#[tauri::command]
pub async fn update_canvas_preset(
    event_id: Uuid,
    preset_id: Uuid,
    preset: CanvasPresetInput,
    state: State<'_, AppState>,
) -> Result<CanvasPreset, String> {
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    let existing = event
        .canvas_presets.iter_mut().find(|p| p.id == preset_id)
        .ok_or_else(|| format!("canvas preset {preset_id} not found"))?;
    preset.apply(existing);
    let updated = existing.clone();
    state.store.save(&event).map_err(|e| e.to_string())?;
    Ok(updated)
}

#[tauri::command]
pub async fn delete_canvas_preset(
    event_id: Uuid,
    preset_id: Uuid,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    event.canvas_presets.retain(|p| p.id != preset_id);
    state.store.save(&event).map_err(|e| e.to_string())
}
