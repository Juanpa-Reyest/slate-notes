//! Coordinator that ties the notes service and the vault together, so a note
//! marked protected is stored encrypted and revealed only while the vault is
//! unlocked. Neither the notes service nor the vault depends on the other; this
//! orchestrator is the single place that knows about both.

use crate::app::notes_service::NotesService;
use crate::app::vault_service::VaultService;
use crate::domain::note::{CreateNoteInput, Note, NoteError, UpdateNoteInput};
use crate::domain::vault::{VaultError, VaultStatus};
use crate::ports::cipher::Cipher;
use crate::ports::clock::Clock;
use crate::ports::note_repository::NoteRepository;
use crate::ports::vault_repository::VaultRepository;

#[derive(Debug)]
pub enum SecureNotesError {
    Note(NoteError),
    Vault(VaultError),
}

impl From<NoteError> for SecureNotesError {
    fn from(error: NoteError) -> Self {
        SecureNotesError::Note(error)
    }
}

impl From<VaultError> for SecureNotesError {
    fn from(error: VaultError) -> Self {
        SecureNotesError::Vault(error)
    }
}

impl std::fmt::Display for SecureNotesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecureNotesError::Note(error) => write!(f, "{error}"),
            SecureNotesError::Vault(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for SecureNotesError {}

pub struct SecureNotesService<NR: NoteRepository, VR, C, K> {
    notes: NotesService<NR>,
    vault: VaultService<VR, C, K>,
}

impl<NR, VR, C, K> SecureNotesService<NR, VR, C, K>
where
    NR: NoteRepository,
    VR: VaultRepository,
    C: Cipher,
    K: Clock,
{
    pub fn new(notes: NotesService<NR>, vault: VaultService<VR, C, K>) -> Self {
        Self { notes, vault }
    }

    // --- Vault control ---

    pub fn vault_status(&mut self) -> Result<VaultStatus, SecureNotesError> {
        Ok(self.vault.status()?)
    }

    pub fn create_vault(&mut self, passphrase: &str) -> Result<(), SecureNotesError> {
        Ok(self.vault.create(passphrase)?)
    }

    pub fn unlock_vault(&mut self, passphrase: &str) -> Result<(), SecureNotesError> {
        Ok(self.vault.unlock(passphrase)?)
    }

    pub fn lock_vault(&mut self) {
        self.vault.lock();
    }

    // --- Notes ---

    pub fn create_note(&mut self, input: CreateNoteInput) -> Result<Note, SecureNotesError> {
        Ok(self.notes.create_note(input)?)
    }

    /// List all notes, revealing protected content when unlocked and blanking it
    /// when locked (so neither plaintext nor ciphertext leaks to the UI).
    pub fn list_notes(&mut self) -> Result<Vec<Note>, SecureNotesError> {
        let notes = self.notes.list_notes()?;
        Ok(notes.into_iter().map(|note| self.present(note)).collect())
    }

    /// Encrypt an existing note's content and mark it protected. Requires unlock.
    pub fn protect_note(&mut self, id: &str) -> Result<Note, SecureNotesError> {
        let note = self.notes.get_note(id)?;
        let sealed = self.vault.protect(&note.content)?;
        let updated = self.notes.set_protection(id, true, sealed)?;
        Ok(self.present(updated))
    }

    /// Decrypt a protected note back to plaintext and clear the flag. Requires unlock.
    pub fn unprotect_note(&mut self, id: &str) -> Result<Note, SecureNotesError> {
        let note = self.notes.get_note(id)?;
        let plaintext = self.vault.reveal(&note.content)?;
        let updated = self.notes.set_protection(id, false, plaintext)?;
        Ok(updated)
    }

    /// Search notes. Protected notes match by content only while unlocked (their
    /// content is revealed first); when locked they match metadata only.
    pub fn search_notes(&mut self, query: &str) -> Result<Vec<Note>, SecureNotesError> {
        let notes = self.list_notes()?;
        Ok(notes
            .into_iter()
            .filter(|note| note.matches_query(query))
            .collect())
    }

    /// Update a note. For a protected note the UI edits plaintext, so we re-seal
    /// the new content before persisting (requires the vault to be unlocked).
    pub fn update_note(&mut self, input: UpdateNoteInput) -> Result<Note, SecureNotesError> {
        let existing = self.notes.get_note(&input.id)?;
        if existing.is_protected {
            let mut sealed_input = input;
            sealed_input.content = self.vault.protect(&sealed_input.content)?;
            let updated = self.notes.update_note(sealed_input)?;
            Ok(self.present(updated))
        } else {
            Ok(self.notes.update_note(input)?)
        }
    }

    pub fn toggle_favorite(&mut self, id: &str) -> Result<Note, SecureNotesError> {
        let note = self.notes.toggle_favorite(id)?;
        Ok(self.present(note))
    }

    pub fn archive_note(&mut self, id: &str) -> Result<Note, SecureNotesError> {
        let note = self.notes.archive_note(id)?;
        Ok(self.present(note))
    }

    pub fn delete_note(&mut self, id: &str) -> Result<(), SecureNotesError> {
        Ok(self.notes.delete_note(id)?)
    }

    /// Build a Markdown document for a note, revealing protected content when the
    /// vault is unlocked (and failing if it is locked). Returns a suggested
    /// filename and the document body.
    pub fn export_markdown(&mut self, id: &str) -> Result<(String, String), SecureNotesError> {
        let note = self.notes.get_note(id)?;
        let content = if note.is_protected {
            self.vault.reveal(&note.content)?
        } else {
            note.content.clone()
        };

        let title = note.title.trim();
        let title = if title.is_empty() { "untitled" } else { title };
        let document = format!("# {title}\n\n{content}\n");
        let filename = format!("{}.md", slugify(title));
        Ok((filename, document))
    }

    /// Replace a protected note's stored (sealed) content with plaintext when the
    /// vault is unlocked, or blank it when locked, before handing it to the UI.
    fn present(&mut self, mut note: Note) -> Note {
        if note.is_protected {
            note.content = self.vault.reveal(&note.content).unwrap_or_default();
        }
        note
    }
}

/// Turn a note title into a safe, lowercase, dash-separated filename stem.
fn slugify(value: &str) -> String {
    let parts: Vec<&str> = value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect();

    if parts.is_empty() {
        "untitled".to_string()
    } else {
        parts.join("-").to_ascii_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;
    use crate::infra::memory_note_repository::MemoryNoteRepository;
    use crate::infra::memory_vault_repository::MemoryVaultRepository;
    use crate::infra::xchacha_cipher::XChaChaCipher;

    #[derive(Clone)]
    struct FakeClock(Rc<Cell<u64>>);

    impl Clock for FakeClock {
        fn now_secs(&self) -> u64 {
            self.0.get()
        }
    }

    fn secure() -> SecureNotesService<MemoryNoteRepository, MemoryVaultRepository, XChaChaCipher, FakeClock>
    {
        let notes = NotesService::new(MemoryNoteRepository::default()).unwrap();
        let vault = VaultService::new(
            MemoryVaultRepository::default(),
            XChaChaCipher::new(),
            FakeClock(Rc::new(Cell::new(0))),
            300,
        );
        SecureNotesService::new(notes, vault)
    }

    fn input(title: &str, content: &str) -> CreateNoteInput {
        CreateNoteInput {
            title: title.to_string(),
            content: content.to_string(),
            category: None,
        }
    }

    #[test]
    fn protect_then_list_reveals_plaintext_while_unlocked() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "my secret thoughts")).unwrap();

        app.protect_note(&note.id).unwrap();

        let listed = app.list_notes().unwrap();
        let protected = listed.iter().find(|n| n.id == note.id).unwrap();
        assert!(protected.is_protected);
        assert_eq!(protected.content, "my secret thoughts");
    }

    #[test]
    fn protected_content_is_blanked_when_locked() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "plaintext-marker")).unwrap();
        app.protect_note(&note.id).unwrap();

        app.lock_vault();

        let listed = app.list_notes().unwrap();
        let protected = listed.iter().find(|n| n.id == note.id).unwrap();
        assert!(protected.is_protected);
        // Never leak plaintext or ciphertext to the UI when locked.
        assert_ne!(protected.content, "plaintext-marker");
        assert_eq!(protected.content, "");
    }

    #[test]
    fn unprotect_restores_plaintext_and_clears_flag() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id).unwrap();

        let restored = app.unprotect_note(&note.id).unwrap();
        assert!(!restored.is_protected);
        assert_eq!(restored.content, "secret");

        let listed = app.list_notes().unwrap();
        let plain = listed.iter().find(|n| n.id == note.id).unwrap();
        assert!(!plain.is_protected);
        assert_eq!(plain.content, "secret");
    }

    #[test]
    fn protect_requires_an_unlocked_vault() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.lock_vault();

        assert!(matches!(
            app.protect_note(&note.id),
            Err(SecureNotesError::Vault(VaultError::Locked))
        ));
    }

    #[test]
    fn export_markdown_builds_document_and_filename() {
        let mut app = secure();
        let note = app.create_note(input("My Great Note", "body text")).unwrap();

        let (filename, document) = app.export_markdown(&note.id).unwrap();

        assert_eq!(filename, "my-great-note.md");
        assert!(document.starts_with("# My Great Note\n\nbody text"));
    }

    #[test]
    fn export_markdown_reveals_protected_content_when_unlocked() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Secret", "classified intel")).unwrap();
        app.protect_note(&note.id).unwrap();

        let (_, document) = app.export_markdown(&note.id).unwrap();
        assert!(document.contains("classified intel"));
    }

    #[test]
    fn export_markdown_fails_for_locked_protected_note() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Secret", "classified intel")).unwrap();
        app.protect_note(&note.id).unwrap();
        app.lock_vault();

        assert!(matches!(
            app.export_markdown(&note.id),
            Err(SecureNotesError::Vault(VaultError::Locked))
        ));
    }
}
