use crate::domain::note::Note;
use crate::domain::vault::VaultStatus;
use crate::state::AppState;
use serde::Deserialize;
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PassphraseInput {
    passphrase: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteIdInput {
    id: String,
}

fn unavailable() -> String {
    "Notes state is unavailable.".to_string()
}

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> Result<VaultStatus, String> {
    state
        .notes
        .lock()
        .map_err(|_| unavailable())?
        .vault_status()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_vault(
    input: PassphraseInput,
    state: State<'_, AppState>,
) -> Result<VaultStatus, String> {
    let mut guard = state.notes.lock().map_err(|_| unavailable())?;
    guard
        .create_vault(&input.passphrase)
        .map_err(|error| error.to_string())?;
    guard.vault_status().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn unlock_vault(
    input: PassphraseInput,
    state: State<'_, AppState>,
) -> Result<VaultStatus, String> {
    let mut guard = state.notes.lock().map_err(|_| unavailable())?;
    guard
        .unlock_vault(&input.passphrase)
        .map_err(|error| error.to_string())?;
    guard.vault_status().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn lock_vault(state: State<'_, AppState>) -> Result<VaultStatus, String> {
    let mut guard = state.notes.lock().map_err(|_| unavailable())?;
    guard.lock_vault();
    guard.vault_status().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn protect_note(input: NoteIdInput, state: State<'_, AppState>) -> Result<Note, String> {
    state
        .notes
        .lock()
        .map_err(|_| unavailable())?
        .protect_note(&input.id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn unprotect_note(input: NoteIdInput, state: State<'_, AppState>) -> Result<Note, String> {
    state
        .notes
        .lock()
        .map_err(|_| unavailable())?
        .unprotect_note(&input.id)
        .map_err(|error| error.to_string())
}
