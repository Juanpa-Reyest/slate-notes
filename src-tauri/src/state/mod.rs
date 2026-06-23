//! Application state and storage selection.
//!
//! Storage is volatile in DEBUG builds and persistent in RELEASE builds. To let
//! both back the same `AppState` without leaking generics into the command
//! layer, the secure-notes service is held behind the [`SecureNotes`] trait
//! object, which mirrors the concrete `SecureNotesService` surface used by the
//! commands.

use crate::app::notes_service::NotesService;
use crate::app::secure_notes::{SecureNotesError, SecureNotesService};
use crate::app::vault_service::VaultService;
use crate::domain::note::{CreateNoteInput, Note, UpdateNoteInput};
use crate::domain::vault::RecoveryStatus;
use crate::infra::sqlite_note_repository::SqliteNoteRepository;
use crate::infra::sqlite_vault_repository::SqliteVaultRepository;
use crate::infra::xchacha_cipher::XChaChaCipher;
use crate::ports::cipher::Cipher;
use crate::ports::note_repository::NoteRepository;
use crate::ports::vault_repository::VaultRepository;
use std::path::Path;
use std::sync::Mutex;

/// The behaviour the command layer needs from the secure-notes service,
/// independent of which repositories back it.
pub trait SecureNotes: Send {
    fn recovery_status(&self) -> Result<RecoveryStatus, SecureNotesError>;
    fn set_up_recovery(&mut self, master_pass: &str) -> Result<(), SecureNotesError>;
    fn clear_active(&mut self);
    fn create_note(&mut self, input: CreateNoteInput) -> Result<Note, SecureNotesError>;
    fn list_notes(&mut self) -> Result<Vec<Note>, SecureNotesError>;
    fn protect_note(&mut self, id: &str, note_pass: &str) -> Result<Note, SecureNotesError>;
    fn reveal_note(&mut self, id: &str, note_pass: &str) -> Result<Note, SecureNotesError>;
    fn recover_note(&mut self, id: &str, master_pass: &str) -> Result<Note, SecureNotesError>;
    fn unprotect_note(&mut self, id: &str, note_pass: &str) -> Result<Note, SecureNotesError>;
    fn search_notes(&mut self, query: &str) -> Result<Vec<Note>, SecureNotesError>;
    fn update_note(&mut self, input: UpdateNoteInput) -> Result<Note, SecureNotesError>;
    fn toggle_favorite(&mut self, id: &str) -> Result<Note, SecureNotesError>;
    fn archive_note(&mut self, id: &str) -> Result<Note, SecureNotesError>;
    fn delete_note(&mut self, id: &str) -> Result<(), SecureNotesError>;
    fn export_markdown(&mut self, id: &str) -> Result<(String, String), SecureNotesError>;
}

impl<NR, VR, C> SecureNotes for SecureNotesService<NR, VR, C>
where
    NR: NoteRepository + Send,
    VR: VaultRepository + Send,
    C: Cipher + Send,
{
    fn recovery_status(&self) -> Result<RecoveryStatus, SecureNotesError> {
        SecureNotesService::recovery_status(self)
    }
    fn set_up_recovery(&mut self, master_pass: &str) -> Result<(), SecureNotesError> {
        SecureNotesService::set_up_recovery(self, master_pass)
    }
    fn clear_active(&mut self) {
        SecureNotesService::clear_active(self)
    }
    fn create_note(&mut self, input: CreateNoteInput) -> Result<Note, SecureNotesError> {
        SecureNotesService::create_note(self, input)
    }
    fn list_notes(&mut self) -> Result<Vec<Note>, SecureNotesError> {
        SecureNotesService::list_notes(self)
    }
    fn protect_note(&mut self, id: &str, note_pass: &str) -> Result<Note, SecureNotesError> {
        SecureNotesService::protect_note(self, id, note_pass)
    }
    fn reveal_note(&mut self, id: &str, note_pass: &str) -> Result<Note, SecureNotesError> {
        SecureNotesService::reveal_note(self, id, note_pass)
    }
    fn recover_note(&mut self, id: &str, master_pass: &str) -> Result<Note, SecureNotesError> {
        SecureNotesService::recover_note(self, id, master_pass)
    }
    fn unprotect_note(&mut self, id: &str, note_pass: &str) -> Result<Note, SecureNotesError> {
        SecureNotesService::unprotect_note(self, id, note_pass)
    }
    fn search_notes(&mut self, query: &str) -> Result<Vec<Note>, SecureNotesError> {
        SecureNotesService::search_notes(self, query)
    }
    fn update_note(&mut self, input: UpdateNoteInput) -> Result<Note, SecureNotesError> {
        SecureNotesService::update_note(self, input)
    }
    fn toggle_favorite(&mut self, id: &str) -> Result<Note, SecureNotesError> {
        SecureNotesService::toggle_favorite(self, id)
    }
    fn archive_note(&mut self, id: &str) -> Result<Note, SecureNotesError> {
        SecureNotesService::archive_note(self, id)
    }
    fn delete_note(&mut self, id: &str) -> Result<(), SecureNotesError> {
        SecureNotesService::delete_note(self, id)
    }
    fn export_markdown(&mut self, id: &str) -> Result<(String, String), SecureNotesError> {
        SecureNotesService::export_markdown(self, id)
    }
}

pub struct AppState {
    pub notes: Mutex<Box<dyn SecureNotes>>,
}

impl AppState {
    /// Persistent SQLite-backed state (RELEASE builds).
    #[cfg_attr(all(debug_assertions, not(test)), allow(dead_code))]
    pub fn sqlite(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();

        let note_repository =
            SqliteNoteRepository::open(path).map_err(|error| error.to_string())?;
        let notes = NotesService::new(note_repository).map_err(|error| error.to_string())?;

        let vault_repository =
            SqliteVaultRepository::open(path).map_err(|error| error.to_string())?;
        let vault = VaultService::new(vault_repository, XChaChaCipher::new());

        Ok(Self {
            notes: Mutex::new(Box::new(SecureNotesService::new(notes, vault))),
        })
    }

    /// Volatile in-memory state (DEBUG builds). Nothing is persisted; everything
    /// is wiped when the app closes.
    #[cfg(any(test, debug_assertions))]
    pub fn memory() -> Result<Self, String> {
        use crate::infra::memory_note_repository::MemoryNoteRepository;
        use crate::infra::memory_vault_repository::MemoryVaultRepository;

        let notes = NotesService::new(MemoryNoteRepository::default())
            .map_err(|error| error.to_string())?;
        let vault = VaultService::new(MemoryVaultRepository::default(), XChaChaCipher::new());

        Ok(Self {
            notes: Mutex::new(Box::new(SecureNotesService::new(notes, vault))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn unique_db_path() -> std::path::PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("slate-state-{}-{}.sqlite", std::process::id(), nonce))
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
    }

    #[test]
    fn fresh_app_starts_with_no_notes() {
        let path = unique_db_path();
        cleanup(&path);

        let state = AppState::sqlite(&path).expect("app state should initialize");
        let notes = state
            .notes
            .lock()
            .expect("notes mutex should lock")
            .list_notes()
            .expect("list should succeed");

        assert!(
            notes.is_empty(),
            "a fresh install must start blank, found {} seeded notes",
            notes.len()
        );

        cleanup(&path);
    }

    #[test]
    fn memory_state_starts_blank_and_supports_recovery_setup() {
        let mut state = AppState::memory().expect("memory state should initialize");
        let guard = state.notes.get_mut().expect("mutex");
        assert!(guard.list_notes().unwrap().is_empty());
        assert!(!guard.recovery_status().unwrap().recovery_initialized);
        guard.set_up_recovery("master-pass").unwrap();
        assert!(guard.recovery_status().unwrap().recovery_initialized);
    }
}
