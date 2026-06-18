use crate::domain::note::{CreateNoteInput, Note, UpdateNoteInput};
use crate::state::AppState;
use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchInput {
    query: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteIdInput {
    id: String,
}

#[tauri::command]
pub fn create_note(input: CreateNoteInput, state: State<'_, AppState>) -> Result<Note, String> {
    state
        .notes
        .lock()
        .map_err(|_| "Notes state is unavailable.".to_string())?
        .create_note(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_notes(state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    state
        .notes
        .lock()
        .map_err(|_| "Notes state is unavailable.".to_string())?
        .list_notes()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn search_notes(input: SearchInput, state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    state
        .notes
        .lock()
        .map_err(|_| "Notes state is unavailable.".to_string())?
        .search_notes(&input.query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_note(input: UpdateNoteInput, state: State<'_, AppState>) -> Result<Note, String> {
    state
        .notes
        .lock()
        .map_err(|_| "Notes state is unavailable.".to_string())?
        .update_note(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn toggle_favorite(input: NoteIdInput, state: State<'_, AppState>) -> Result<Note, String> {
    state
        .notes
        .lock()
        .map_err(|_| "Notes state is unavailable.".to_string())?
        .toggle_favorite(&input.id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn archive_note(input: NoteIdInput, state: State<'_, AppState>) -> Result<Note, String> {
    state
        .notes
        .lock()
        .map_err(|_| "Notes state is unavailable.".to_string())?
        .archive_note(&input.id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_note(input: NoteIdInput, state: State<'_, AppState>) -> Result<(), String> {
    state
        .notes
        .lock()
        .map_err(|_| "Notes state is unavailable.".to_string())?
        .delete_note(&input.id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn export_note(
    input: NoteIdInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let (filename, content) = state
        .notes
        .lock()
        .map_err(|_| "Notes state is unavailable.".to_string())?
        .export_markdown(&input.id)
        .map_err(|error| error.to_string())?;

    let directory = app
        .path()
        .download_dir()
        .or_else(|_| app.path().home_dir())
        .map_err(|error| error.to_string())?;
    let path = directory.join(filename);

    std::fs::write(&path, content).map_err(|error| error.to_string())?;

    Ok(path.to_string_lossy().into_owned())
}
