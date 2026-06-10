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

#[tauri::command]
pub async fn list_canvas_presets(
    event_id: Uuid,
    state: State<'_, AppState>,
) -> Result<Vec<CanvasPreset>, String> {
    let event = state.store.load(event_id).map_err(|e| e.to_string())?;
    Ok(event.canvas_presets)
}

#[tauri::command]
pub async fn create_canvas_preset(
    event_id: Uuid,
    preset: CanvasPresetInput,
    state: State<'_, AppState>,
) -> Result<CanvasPreset, String> {
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    let preset = CanvasPreset {
        id: Uuid::new_v4(),
        name: preset.name,
        canvas_width_px: preset.canvas_width_px,
        canvas_height_px: preset.canvas_height_px,
        photos_per_canvas: preset.photos_per_canvas,
        dpi: preset.dpi,
        margin_px: preset.margin_px,
        cols: preset.cols,
        rows: preset.rows,
    };
    event.canvas_presets.push(preset.clone());
    state.store.save(&event).map_err(|e| e.to_string())?;
    Ok(preset)
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
