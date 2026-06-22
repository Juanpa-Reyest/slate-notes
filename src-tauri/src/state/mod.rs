use crate::app::notes_service::NotesService;
use crate::app::secure_notes::SecureNotesService;
use crate::app::vault_service::VaultService;
use crate::infra::sqlite_note_repository::SqliteNoteRepository;
use crate::infra::sqlite_vault_repository::SqliteVaultRepository;
use crate::infra::xchacha_cipher::XChaChaCipher;
use std::path::Path;
use std::sync::Mutex;

pub type AppSecureNotes =
    SecureNotesService<SqliteNoteRepository, SqliteVaultRepository, XChaChaCipher>;

pub struct AppState {
    pub notes: Mutex<AppSecureNotes>,
}

impl AppState {
    pub fn sqlite(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();

        let note_repository =
            SqliteNoteRepository::open(path).map_err(|error| error.to_string())?;
        let notes = NotesService::new(note_repository).map_err(|error| error.to_string())?;

        let vault_repository =
            SqliteVaultRepository::open(path).map_err(|error| error.to_string())?;
        let vault = VaultService::new(vault_repository, XChaChaCipher::new());

        Ok(Self {
            notes: Mutex::new(SecureNotesService::new(notes, vault)),
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
}
