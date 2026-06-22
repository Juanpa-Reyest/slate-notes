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

/// Input for a per-note protected operation: the note id plus the passphrase
/// supplied at that moment. The passphrase is never logged.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotePassphraseInput {
    id: String,
    passphrase: String,
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
pub fn reveal_note(input: NotePassphraseInput, state: State<'_, AppState>) -> Result<Note, String> {
    let id = input.id.clone();
    // The passphrase is never logged — only the note id and the outcome.
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| unavailable())?
            .reveal_note(&input.id, &input.passphrase)
            .map_err(|error| error.to_string())
    })();
    log_outcome("reveal_note", Some(&id), result)
}

#[tauri::command]
pub fn clear_active(state: State<'_, AppState>) -> Result<VaultStatus, String> {
    let result = (|| {
        let mut guard = state.notes.lock().map_err(|_| unavailable())?;
        guard.clear_active();
        guard.vault_status().map_err(|error| error.to_string())
    })();
    log_outcome("clear_active", None, result)
}

#[tauri::command]
pub fn protect_note(input: NotePassphraseInput, state: State<'_, AppState>) -> Result<Note, String> {
    let id = input.id.clone();
    // The passphrase is never logged — only the note id and the outcome.
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| unavailable())?
            .protect_note(&input.id, &input.passphrase)
            .map_err(|error| error.to_string())
    })();
    log_outcome("protect_note", Some(&id), result)
}

#[tauri::command]
pub fn unprotect_note(
    input: NotePassphraseInput,
    state: State<'_, AppState>,
) -> Result<Note, String> {
    let id = input.id.clone();
    // The passphrase is never logged — only the note id and the outcome.
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| unavailable())?
            .unprotect_note(&input.id, &input.passphrase)
            .map_err(|error| error.to_string())
    })();
    log_outcome("unprotect_note", Some(&id), result)
}
