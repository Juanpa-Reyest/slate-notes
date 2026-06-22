use crate::domain::note::{CreateNoteInput, Note, UpdateNoteInput};
use crate::state::AppState;
use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

use super::{log_outcome, log_read_error};

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
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| "Notes state is unavailable.".to_string())?
            .create_note(input)
            .map_err(|error| error.to_string())
    })();
    // The id is only known after creation succeeds.
    let id = result.as_ref().ok().map(|note| note.id.clone());
    log_outcome("create_note", id.as_deref(), result)
}

#[tauri::command]
pub fn list_notes(state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| "Notes state is unavailable.".to_string())?
            .list_notes()
            .map_err(|error| error.to_string())
    })();
    log_read_error("list_notes", result)
}

#[tauri::command]
pub fn search_notes(input: SearchInput, state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    // The query text is intentionally not logged (it may be sensitive).
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| "Notes state is unavailable.".to_string())?
            .search_notes(&input.query)
            .map_err(|error| error.to_string())
    })();
    log_read_error("search_notes", result)
}

#[tauri::command]
pub fn update_note(input: UpdateNoteInput, state: State<'_, AppState>) -> Result<Note, String> {
    let id = input.id.clone();
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| "Notes state is unavailable.".to_string())?
            .update_note(input)
            .map_err(|error| error.to_string())
    })();
    log_outcome("update_note", Some(&id), result)
}

#[tauri::command]
pub fn toggle_favorite(input: NoteIdInput, state: State<'_, AppState>) -> Result<Note, String> {
    let id = input.id.clone();
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| "Notes state is unavailable.".to_string())?
            .toggle_favorite(&input.id)
            .map_err(|error| error.to_string())
    })();
    log_outcome("toggle_favorite", Some(&id), result)
}

#[tauri::command]
pub fn archive_note(input: NoteIdInput, state: State<'_, AppState>) -> Result<Note, String> {
    let id = input.id.clone();
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| "Notes state is unavailable.".to_string())?
            .archive_note(&input.id)
            .map_err(|error| error.to_string())
    })();
    log_outcome("archive_note", Some(&id), result)
}

#[tauri::command]
pub fn delete_note(input: NoteIdInput, state: State<'_, AppState>) -> Result<(), String> {
    let id = input.id.clone();
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| "Notes state is unavailable.".to_string())?
            .delete_note(&input.id)
            .map_err(|error| error.to_string())
    })();
    log_outcome("delete_note", Some(&id), result)
}

#[tauri::command]
pub fn export_note(
    input: NoteIdInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let id = input.id.clone();
    // The destination path (derived from the note title) is not logged.
    let result = (|| {
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
    })();
    log_outcome("export_note", Some(&id), result)
}
