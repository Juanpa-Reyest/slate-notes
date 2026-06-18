use crate::app::notes_service::NotesService;
use crate::app::secure_notes::SecureNotesService;
use crate::app::vault_service::VaultService;
use crate::infra::sqlite_note_repository::SqliteNoteRepository;
use crate::infra::sqlite_vault_repository::SqliteVaultRepository;
use crate::infra::system_clock::SystemClock;
use crate::infra::xchacha_cipher::XChaChaCipher;
use std::path::Path;
use std::sync::Mutex;

/// Protected notes auto-lock after this many seconds of inactivity.
const AUTO_LOCK_SECS: u64 = 300;

pub type AppSecureNotes =
    SecureNotesService<SqliteNoteRepository, SqliteVaultRepository, XChaChaCipher, SystemClock>;

pub struct AppState {
    pub notes: Mutex<AppSecureNotes>,
}

impl AppState {
    pub fn sqlite(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();

        let note_repository =
            SqliteNoteRepository::open(path).map_err(|error| error.to_string())?;
        let mut notes = NotesService::new(note_repository).map_err(|error| error.to_string())?;
        notes.seed().map_err(|error| error.to_string())?;

        let vault_repository =
            SqliteVaultRepository::open(path).map_err(|error| error.to_string())?;
        let vault = VaultService::new(
            vault_repository,
            XChaChaCipher::new(),
            SystemClock::new(),
            AUTO_LOCK_SECS,
        );

        Ok(Self {
            notes: Mutex::new(SecureNotesService::new(notes, vault)),
        })
    }
}
