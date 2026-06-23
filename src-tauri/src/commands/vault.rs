use crate::domain::note::Note;
use crate::domain::vault::RecoveryStatus;
use crate::state::AppState;
use serde::Deserialize;
use tauri::State;

use super::{log_outcome, log_read_error};

/// Input carrying a single passphrase (the master passphrase for recovery
/// setup). The passphrase is never logged.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PassphraseInput {
    passphrase: String,
}

/// Input for a per-note protected operation: the note id plus the passphrase
/// supplied at that moment (the note password, or the master passphrase for
/// `recover_note`). The passphrase is never logged.
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
pub fn recovery_status(state: State<'_, AppState>) -> Result<RecoveryStatus, String> {
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| unavailable())?
            .recovery_status()
            .map_err(|error| error.to_string())
    })();
    log_read_error("recovery_status", result)
}

#[tauri::command]
pub fn set_up_recovery(
    input: PassphraseInput,
    state: State<'_, AppState>,
) -> Result<RecoveryStatus, String> {
    // The passphrase is never logged — only whether recovery setup succeeded.
    let result = (|| {
        let mut guard = state.notes.lock().map_err(|_| unavailable())?;
        guard
            .set_up_recovery(&input.passphrase)
            .map_err(|error| error.to_string())?;
        guard.recovery_status().map_err(|error| error.to_string())
    })();
    log_outcome("set_up_recovery", None, result)
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
pub fn clear_active(state: State<'_, AppState>) -> Result<RecoveryStatus, String> {
    let result = (|| {
        let mut guard = state.notes.lock().map_err(|_| unavailable())?;
        guard.clear_active();
        guard.recovery_status().map_err(|error| error.to_string())
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

#[tauri::command]
pub fn recover_note(
    input: NotePassphraseInput,
    state: State<'_, AppState>,
) -> Result<Note, String> {
    let id = input.id.clone();
    // The master passphrase is never logged — only the note id and the outcome.
    let result = (|| {
        state
            .notes
            .lock()
            .map_err(|_| unavailable())?
            .recover_note(&input.id, &input.passphrase)
            .map_err(|error| error.to_string())
    })();
    log_outcome("recover_note", Some(&id), result)
}
