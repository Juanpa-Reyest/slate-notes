use crate::domain::note::Note;
use crate::domain::vault::VaultStatus;
use crate::state::AppState;
use serde::Deserialize;
use tauri::State;

use super::{log_outcome, log_read_error};

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
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| unavailable())?
            .vault_status()
            .map_err(|error| error.to_string())
    })();
    log_read_error("vault_status", result)
}

#[tauri::command]
pub fn create_vault(
    input: PassphraseInput,
    state: State<'_, AppState>,
) -> Result<VaultStatus, String> {
    // The passphrase is never logged — only whether vault creation succeeded.
    let result = (|| {
        let mut guard = state.notes.lock().map_err(|_| unavailable())?;
        guard
            .create_vault(&input.passphrase)
            .map_err(|error| error.to_string())?;
        guard.vault_status().map_err(|error| error.to_string())
    })();
    log_outcome("create_vault", None, result)
}

#[tauri::command]
pub fn unlock_vault(
    input: PassphraseInput,
    state: State<'_, AppState>,
) -> Result<VaultStatus, String> {
    // The passphrase is never logged — only whether the unlock succeeded.
    let result = (|| {
        let mut guard = state.notes.lock().map_err(|_| unavailable())?;
        guard
            .unlock_vault(&input.passphrase)
            .map_err(|error| error.to_string())?;
        guard.vault_status().map_err(|error| error.to_string())
    })();
    log_outcome("unlock_vault", None, result)
}

#[tauri::command]
pub fn lock_vault(state: State<'_, AppState>) -> Result<VaultStatus, String> {
    let result = (|| {
        let mut guard = state.notes.lock().map_err(|_| unavailable())?;
        guard.lock_vault();
        guard.vault_status().map_err(|error| error.to_string())
    })();
    log_outcome("lock_vault", None, result)
}

#[tauri::command]
pub fn protect_note(input: NoteIdInput, state: State<'_, AppState>) -> Result<Note, String> {
    let id = input.id.clone();
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| unavailable())?
            .protect_note(&input.id)
            .map_err(|error| error.to_string())
    })();
    log_outcome("protect_note", Some(&id), result)
}

#[tauri::command]
pub fn unprotect_note(input: NoteIdInput, state: State<'_, AppState>) -> Result<Note, String> {
    let id = input.id.clone();
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| unavailable())?
            .unprotect_note(&input.id)
            .map_err(|error| error.to_string())
    })();
    log_outcome("unprotect_note", Some(&id), result)
}
