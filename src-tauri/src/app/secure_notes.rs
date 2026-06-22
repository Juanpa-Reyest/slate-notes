//! Coordinator that ties the notes service and the vault together under STRICT
//! PER-NOTE authentication. A protected note is stored encrypted and is revealed
//! only when its passphrase is supplied at that moment. There is no persistent
//! unlocked session: the only key held in memory is the transient key for the
//! note the user currently has open, so autosave can re-seal edits without
//! re-prompting per keystroke. That key is cleared when the user navigates away.

use crate::app::notes_service::NotesService;
use crate::app::vault_service::VaultService;
use crate::domain::encryption::DerivedKey;
use crate::domain::note::{CreateNoteInput, Note, NoteError, UpdateNoteInput};
use crate::domain::vault::{VaultError, VaultStatus};
use crate::ports::cipher::Cipher;
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

pub struct SecureNotesService<NR: NoteRepository, VR, C> {
    notes: NotesService<NR>,
    vault: VaultService<VR, C>,
    /// The transient key for the currently-open protected note: (note id, key).
    /// This is the ONLY decrypted-key state the app holds. It exists so autosave
    /// can re-seal edits to the open note without re-prompting per keystroke.
    active: Option<(String, DerivedKey)>,
}

impl<NR, VR, C> SecureNotesService<NR, VR, C>
where
    NR: NoteRepository,
    VR: VaultRepository,
    C: Cipher,
{
    pub fn new(notes: NotesService<NR>, vault: VaultService<VR, C>) -> Self {
        Self {
            notes,
            vault,
            active: None,
        }
    }

    // --- Vault control ---

    /// Vault status for the UI. `unlocked` now means "a protected note is open"
    /// (i.e. a transient key is held), keeping the existing shape for the
    /// frontend.
    pub fn vault_status(&self) -> Result<VaultStatus, SecureNotesError> {
        Ok(VaultStatus {
            initialized: self.vault.is_initialized()?,
            unlocked: self.active.is_some(),
        })
    }

    pub fn create_vault(&mut self, passphrase: &str) -> Result<(), SecureNotesError> {
        // Creating the vault derives a key but there is no open note yet, so we
        // do not retain it as the active key.
        let _key = self.vault.create(passphrase)?;
        Ok(())
    }

    /// Drop the transient key for the currently-open protected note. Called when
    /// the user navigates away from it.
    pub fn clear_active(&mut self) {
        self.active = None;
    }

    // --- Notes ---

    pub fn create_note(&mut self, input: CreateNoteInput) -> Result<Note, SecureNotesError> {
        Ok(self.notes.create_note(input)?)
    }

    /// List all notes. Protected notes are ALWAYS blanked (content = ""),
    /// regardless of whether one is currently open, so neither plaintext nor
    /// ciphertext leaks to the UI through the list.
    pub fn list_notes(&mut self) -> Result<Vec<Note>, SecureNotesError> {
        let notes = self.notes.list_notes()?;
        Ok(notes.into_iter().map(blank_if_protected).collect())
    }

    /// Reveal a single protected note by supplying its passphrase. Returns the
    /// note with decrypted plaintext content and retains the derived key as the
    /// active key so subsequent autosaves can re-seal. A non-protected note is
    /// returned unchanged.
    pub fn reveal_note(
        &mut self,
        id: &str,
        passphrase: &str,
    ) -> Result<Note, SecureNotesError> {
        let mut note = self.notes.get_note(id)?;
        if !note.is_protected {
            return Ok(note);
        }

        let key = self.vault.verify_key(passphrase)?;
        let plaintext = self.vault.open_with(&key, &note.content)?;
        note.content = plaintext;
        self.active = Some((id.to_string(), key));
        Ok(note)
    }

    /// Encrypt an existing note's content and mark it protected. The passphrase
    /// is supplied at this moment. The note becomes the active note and is
    /// returned with its plaintext content so the UI can keep showing it.
    pub fn protect_note(
        &mut self,
        id: &str,
        passphrase: &str,
    ) -> Result<Note, SecureNotesError> {
        let note = self.notes.get_note(id)?;
        let key = self.vault.verify_key(passphrase)?;
        let plaintext = note.content.clone();
        let sealed = self.vault.seal_with(&key, &note.content)?;
        let mut updated = self.notes.set_protection(id, true, sealed)?;
        updated.content = plaintext;
        self.active = Some((id.to_string(), key));
        Ok(updated)
    }

    /// Decrypt a protected note back to plaintext and clear the flag. The
    /// passphrase is supplied at this moment.
    pub fn unprotect_note(
        &mut self,
        id: &str,
        passphrase: &str,
    ) -> Result<Note, SecureNotesError> {
        let note = self.notes.get_note(id)?;
        let key = self.vault.verify_key(passphrase)?;
        let plaintext = self.vault.open_with(&key, &note.content)?;
        let updated = self.notes.set_protection(id, false, plaintext)?;
        if self.active_id() == Some(id) {
            self.clear_active();
        }
        Ok(updated)
    }

    /// Search notes over the blanked list, so protected content is never
    /// searchable.
    pub fn search_notes(&mut self, query: &str) -> Result<Vec<Note>, SecureNotesError> {
        let notes = self.list_notes()?;
        Ok(notes
            .into_iter()
            .filter(|note| note.matches_query(query))
            .collect())
    }

    /// Update a note. For a protected note the UI edits plaintext, so we re-seal
    /// the new content before persisting — but ONLY when that exact note is the
    /// active (currently-open) note, whose key we hold transiently. Otherwise
    /// the operation is locked.
    pub fn update_note(&mut self, input: UpdateNoteInput) -> Result<Note, SecureNotesError> {
        let existing = self.notes.get_note(&input.id)?;
        if existing.is_protected {
            let key = match &self.active {
                Some((active_id, key)) if active_id == &input.id => key,
                _ => return Err(VaultError::Locked.into()),
            };
            let mut sealed_input = input;
            sealed_input.content = self.vault.seal_with(key, &sealed_input.content)?;
            let updated = self.notes.update_note(sealed_input)?;
            Ok(blank_if_protected(updated))
        } else {
            Ok(self.notes.update_note(input)?)
        }
    }

    pub fn toggle_favorite(&mut self, id: &str) -> Result<Note, SecureNotesError> {
        Ok(blank_if_protected(self.notes.toggle_favorite(id)?))
    }

    pub fn archive_note(&mut self, id: &str) -> Result<Note, SecureNotesError> {
        Ok(blank_if_protected(self.notes.archive_note(id)?))
    }

    pub fn delete_note(&mut self, id: &str) -> Result<(), SecureNotesError> {
        if self.active_id() == Some(id) {
            self.clear_active();
        }
        Ok(self.notes.delete_note(id)?)
    }

    /// Build a Markdown document for a note. Protected content has no plaintext
    /// available without a passphrase under per-note auth, so a protected note
    /// exports with blanked content (never plaintext or ciphertext).
    pub fn export_markdown(&mut self, id: &str) -> Result<(String, String), SecureNotesError> {
        let note = self.notes.get_note(id)?;
        let content = if note.is_protected {
            String::new()
        } else {
            note.content.clone()
        };

        let title = note.title.trim();
        let title = if title.is_empty() { "untitled" } else { title };
        let document = format!("# {title}\n\n{content}\n");
        let filename = format!("{}.md", slugify(title));
        Ok((filename, document))
    }

    fn active_id(&self) -> Option<&str> {
        self.active.as_ref().map(|(id, _)| id.as_str())
    }
}

/// Blank a protected note's content before handing it to the UI, so neither
/// plaintext nor ciphertext leaks.
fn blank_if_protected(mut note: Note) -> Note {
    if note.is_protected {
        note.content = String::new();
    }
    note
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
    use super::*;
    use crate::infra::memory_note_repository::MemoryNoteRepository;
    use crate::infra::memory_vault_repository::MemoryVaultRepository;
    use crate::infra::xchacha_cipher::XChaChaCipher;

    fn secure() -> SecureNotesService<MemoryNoteRepository, MemoryVaultRepository, XChaChaCipher> {
        let notes = NotesService::new(MemoryNoteRepository::default()).unwrap();
        let vault = VaultService::new(MemoryVaultRepository::default(), XChaChaCipher::new());
        SecureNotesService::new(notes, vault)
    }

    fn input(title: &str, content: &str) -> CreateNoteInput {
        CreateNoteInput {
            title: title.to_string(),
            content: content.to_string(),
            category: None,
        }
    }

    fn update(id: &str, content: &str) -> UpdateNoteInput {
        UpdateNoteInput {
            id: id.to_string(),
            title: "Diary".to_string(),
            content: content.to_string(),
            category: "Inbox".to_string(),
            tags: Vec::new(),
            color: "slate".to_string(),
        }
    }

    #[test]
    fn vault_status_tracks_initialization_and_active_note() {
        let mut app = secure();
        assert_eq!(
            app.vault_status().unwrap(),
            VaultStatus {
                initialized: false,
                unlocked: false
            }
        );

        app.create_vault("master-pass").unwrap();
        // Creating the vault does not open a note.
        assert_eq!(
            app.vault_status().unwrap(),
            VaultStatus {
                initialized: true,
                unlocked: false
            }
        );

        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "master-pass").unwrap();
        // Protecting opens the note (active key held).
        assert!(app.vault_status().unwrap().unlocked);

        app.clear_active();
        assert!(!app.vault_status().unwrap().unlocked);
    }

    #[test]
    fn reveal_note_returns_plaintext_with_correct_passphrase() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "my secret thoughts")).unwrap();
        app.protect_note(&note.id, "master-pass").unwrap();
        app.clear_active();

        let revealed = app.reveal_note(&note.id, "master-pass").unwrap();
        assert!(revealed.is_protected);
        assert_eq!(revealed.content, "my secret thoughts");
        // Revealing makes it the active note.
        assert!(app.vault_status().unwrap().unlocked);
    }

    #[test]
    fn reveal_note_with_wrong_passphrase_errors() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "master-pass").unwrap();
        app.clear_active();

        assert!(matches!(
            app.reveal_note(&note.id, "wrong-pass"),
            Err(SecureNotesError::Vault(VaultError::InvalidPassphrase))
        ));
        // A failed reveal does not open the note.
        assert!(!app.vault_status().unwrap().unlocked);
    }

    #[test]
    fn reveal_non_protected_note_returns_it_unchanged() {
        let mut app = secure();
        let note = app.create_note(input("Public", "plain body")).unwrap();

        let revealed = app.reveal_note(&note.id, "irrelevant").unwrap();
        assert!(!revealed.is_protected);
        assert_eq!(revealed.content, "plain body");
        assert!(!app.vault_status().unwrap().unlocked);
    }

    #[test]
    fn list_notes_always_blanks_protected_content_even_when_active() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "plaintext-marker")).unwrap();
        app.protect_note(&note.id, "master-pass").unwrap();

        // The note is active right now, yet the list must still blank it.
        assert!(app.vault_status().unwrap().unlocked);
        let listed = app.list_notes().unwrap();
        let protected = listed.iter().find(|n| n.id == note.id).unwrap();
        assert!(protected.is_protected);
        assert_eq!(protected.content, "");
    }

    #[test]
    fn update_protected_note_requires_it_to_be_active() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "master-pass").unwrap();
        app.clear_active();

        // No active key -> locked.
        assert!(matches!(
            app.update_note(update(&note.id, "new secret")),
            Err(SecureNotesError::Vault(VaultError::Locked))
        ));
    }

    #[test]
    fn update_protected_note_reseals_when_active_and_roundtrips() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "master-pass").unwrap();

        // Active note: update re-seals the new plaintext.
        let updated = app.update_note(update(&note.id, "new secret")).unwrap();
        assert!(updated.is_protected);
        assert_eq!(updated.content, ""); // blanked on the way out

        // Revealing again returns the new content.
        app.clear_active();
        let revealed = app.reveal_note(&note.id, "master-pass").unwrap();
        assert_eq!(revealed.content, "new secret");
    }

    #[test]
    fn protect_then_reveal_roundtrips() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "my secret thoughts")).unwrap();

        let protected = app.protect_note(&note.id, "master-pass").unwrap();
        assert!(protected.is_protected);
        // protect_note returns plaintext so the UI keeps showing the open note.
        assert_eq!(protected.content, "my secret thoughts");

        app.clear_active();
        let revealed = app.reveal_note(&note.id, "master-pass").unwrap();
        assert_eq!(revealed.content, "my secret thoughts");
    }

    #[test]
    fn protect_with_wrong_passphrase_errors() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();

        assert!(matches!(
            app.protect_note(&note.id, "wrong-pass"),
            Err(SecureNotesError::Vault(VaultError::InvalidPassphrase))
        ));
    }

    #[test]
    fn unprotect_restores_plaintext_and_clears_flag() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "master-pass").unwrap();

        let restored = app.unprotect_note(&note.id, "master-pass").unwrap();
        assert!(!restored.is_protected);
        assert_eq!(restored.content, "secret");
        // The active key for that note is cleared on unprotect.
        assert!(!app.vault_status().unwrap().unlocked);

        let listed = app.list_notes().unwrap();
        let plain = listed.iter().find(|n| n.id == note.id).unwrap();
        assert!(!plain.is_protected);
        assert_eq!(plain.content, "secret");
    }

    #[test]
    fn unprotect_with_wrong_passphrase_errors() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "master-pass").unwrap();

        assert!(matches!(
            app.unprotect_note(&note.id, "wrong-pass"),
            Err(SecureNotesError::Vault(VaultError::InvalidPassphrase))
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
    fn export_markdown_blanks_protected_content() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Secret", "classified intel")).unwrap();
        app.protect_note(&note.id, "master-pass").unwrap();

        let (_, document) = app.export_markdown(&note.id).unwrap();
        // Never leak plaintext or ciphertext through export.
        assert!(!document.contains("classified intel"));
        assert!(document.starts_with("# Secret\n\n\n"));
    }

    #[test]
    fn search_matches_metadata_and_excludes_others() {
        let mut app = secure();
        app.create_note(input("Rust backend", "domain tests")).unwrap();
        app.create_note(input("Shopping", "buy milk")).unwrap();

        let results = app.search_notes("backend").unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust backend");
    }

    #[test]
    fn search_never_matches_protected_content_even_when_active() {
        let mut app = secure();
        app.create_vault("master-pass").unwrap();
        let note = app.create_note(input("Journal", "uniquesecretword")).unwrap();
        app.protect_note(&note.id, "master-pass").unwrap();

        // Even with the note active, its protected content is never searchable.
        assert!(app.vault_status().unwrap().unlocked);
        assert!(app.search_notes("uniquesecretword").unwrap().is_empty());
    }
}
