//! Coordinator that ties the notes service and the recovery vault together
//! under PER-NOTE authentication with asymmetric master recovery.
//!
//! Each protected note carries its own random DEK, wrapped under its own
//! password AND escrowed to the master public key. There is no persistent
//! unlocked session: the only key held in memory is the transient DEK for the
//! note the user currently has open, so autosave can re-seal edits without
//! re-prompting per keystroke. That DEK is cleared when the user navigates away.

use crate::app::notes_service::NotesService;
use crate::app::vault_service::VaultService;
use crate::domain::encryption::{DerivedKey, PAYLOAD_VERSION};
use crate::domain::note::{CreateNoteInput, Note, NoteError, UpdateNoteInput};
use crate::domain::vault::{ProtectedPayload, RecoveryStatus, VaultError};
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
    /// The transient DEK for the currently-open protected note: (note id, DEK).
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

    // --- Recovery control ---

    /// Recovery/auth status for the UI.
    pub fn recovery_status(&self) -> Result<RecoveryStatus, SecureNotesError> {
        Ok(RecoveryStatus {
            recovery_initialized: self.vault.is_initialized()?,
            active_note_open: self.active.is_some(),
        })
    }

    /// Set up master recovery once. Generates the X25519 keypair, seals the
    /// private key under the master passphrase, and persists the record.
    pub fn set_up_recovery(&mut self, master_pass: &str) -> Result<(), SecureNotesError> {
        self.vault.set_up_recovery(master_pass)?;
        Ok(())
    }

    /// Drop the transient DEK for the currently-open protected note. Called when
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

    /// Protect a note. REQUIRES master recovery to be set up first. Generates a
    /// per-note salt and a random DEK; seals the content with the DEK; wraps the
    /// DEK under the note password; escrows the DEK to the master public key.
    /// The note becomes the active note and is returned with plaintext content
    /// so the UI can keep showing it.
    pub fn protect_note(
        &mut self,
        id: &str,
        note_pass: &str,
    ) -> Result<Note, SecureNotesError> {
        // Per-note protection cannot exist without the escrow target.
        if !self.vault.is_initialized()? {
            return Err(VaultError::NotInitialized.into());
        }

        let note = self.notes.get_note(id)?;
        let plaintext = note.content.clone();

        let note_salt = self.vault.generate_salt();
        let dek = self.vault.generate_dek()?;

        let content = self.vault.seal_content(&dek, &plaintext)?;
        let dek_by_pass = self.vault.wrap_dek_by_pass(&dek, note_pass, &note_salt)?;
        let dek_escrow = self.vault.escrow_dek(&dek)?;

        let payload = ProtectedPayload {
            version: PAYLOAD_VERSION,
            note_salt,
            content,
            dek_by_pass,
            dek_escrow,
        };

        let mut updated = self.notes.set_protection(id, true, encode_payload(&payload))?;
        updated.content = plaintext;
        self.active = Some((id.to_string(), dek));
        Ok(updated)
    }

    /// Reveal a protected note by supplying its OWN password. Unwraps the DEK
    /// from `dek_by_pass`, decrypts the content, and holds the DEK as the active
    /// key. A non-protected note is returned unchanged.
    pub fn reveal_note(
        &mut self,
        id: &str,
        note_pass: &str,
    ) -> Result<Note, SecureNotesError> {
        let mut note = self.notes.get_note(id)?;
        if !note.is_protected {
            return Ok(note);
        }

        let payload = decode_payload(&note.content)?;
        let dek = self
            .vault
            .unwrap_dek_by_pass(&payload.dek_by_pass, note_pass, &payload.note_salt)?;
        let plaintext = self.vault.open_content(&dek, &payload.content)?;

        note.content = plaintext;
        self.active = Some((id.to_string(), dek));
        Ok(note)
    }

    /// Recover a protected note whose password was forgotten, using the MASTER
    /// passphrase. Decrypts the master private key, ECIES-decrypts the escrowed
    /// DEK, decrypts the content, then UNPROTECTS the note: the plaintext is
    /// stored back, the flag cleared, and the protected payload removed. This
    /// removes the old note password.
    pub fn recover_note(
        &mut self,
        id: &str,
        master_pass: &str,
    ) -> Result<Note, SecureNotesError> {
        let note = self.notes.get_note(id)?;
        if !note.is_protected {
            return Ok(note);
        }

        let payload = decode_payload(&note.content)?;
        let private_key = self.vault.recover_private_key(master_pass)?;
        let dek = self
            .vault
            .recover_dek_from_escrow(&private_key, &payload.dek_escrow)?;
        let plaintext = self.vault.open_content(&dek, &payload.content)?;

        let updated = self.notes.set_protection(id, false, plaintext)?;
        if self.active_id() == Some(id) {
            self.clear_active();
        }
        Ok(updated)
    }

    /// Decrypt a protected note back to plaintext and clear the flag, using the
    /// note's OWN password.
    pub fn unprotect_note(
        &mut self,
        id: &str,
        note_pass: &str,
    ) -> Result<Note, SecureNotesError> {
        let note = self.notes.get_note(id)?;
        if !note.is_protected {
            return Ok(note);
        }

        let payload = decode_payload(&note.content)?;
        let dek = self
            .vault
            .unwrap_dek_by_pass(&payload.dek_by_pass, note_pass, &payload.note_salt)?;
        let plaintext = self.vault.open_content(&dek, &payload.content)?;

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
    /// the new content with the held active DEK — but ONLY when that exact note
    /// is the active (currently-open) note. The note_salt, dek_by_pass and
    /// dek_escrow are preserved unchanged; only `content` is re-sealed with the
    /// same DEK. Otherwise the operation is locked.
    pub fn update_note(&mut self, input: UpdateNoteInput) -> Result<Note, SecureNotesError> {
        let existing = self.notes.get_note(&input.id)?;
        if existing.is_protected {
            let dek = match &self.active {
                Some((active_id, dek)) if active_id == &input.id => dek,
                _ => return Err(VaultError::Locked.into()),
            };

            let mut payload = decode_payload(&existing.content)?;
            payload.content = self.vault.seal_content(dek, &input.content)?;

            let mut sealed_input = input;
            sealed_input.content = encode_payload(&payload);
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
    /// available without a passphrase, so a protected note exports with blanked
    /// content (never plaintext or ciphertext).
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

/// Serialize a protected payload into the note's content string.
fn encode_payload(payload: &ProtectedPayload) -> String {
    serde_json::to_string(payload).expect("a protected payload always serializes")
}

/// Parse a stored content string into a protected payload.
fn decode_payload(stored: &str) -> Result<ProtectedPayload, VaultError> {
    serde_json::from_str(stored).map_err(|_| VaultError::CorruptPayload)
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
    fn recovery_status_tracks_initialization_and_active_note() {
        let mut app = secure();
        assert_eq!(
            app.recovery_status().unwrap(),
            RecoveryStatus {
                recovery_initialized: false,
                active_note_open: false
            }
        );

        app.set_up_recovery("master-pass").unwrap();
        assert_eq!(
            app.recovery_status().unwrap(),
            RecoveryStatus {
                recovery_initialized: true,
                active_note_open: false
            }
        );

        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "1234").unwrap();
        // Protecting opens the note (active DEK held).
        assert!(app.recovery_status().unwrap().active_note_open);

        app.clear_active();
        assert!(!app.recovery_status().unwrap().active_note_open);
    }

    #[test]
    fn protect_before_recovery_setup_errors() {
        let mut app = secure();
        let note = app.create_note(input("Diary", "secret")).unwrap();

        assert!(matches!(
            app.protect_note(&note.id, "1234"),
            Err(SecureNotesError::Vault(VaultError::NotInitialized))
        ));
    }

    #[test]
    fn protect_then_reveal_roundtrips() {
        let mut app = secure();
        app.set_up_recovery("master-pass").unwrap();
        let note = app.create_note(input("Diary", "my secret thoughts")).unwrap();

        let protected = app.protect_note(&note.id, "1234").unwrap();
        assert!(protected.is_protected);
        // protect_note returns plaintext so the UI keeps showing the open note.
        assert_eq!(protected.content, "my secret thoughts");

        app.clear_active();
        let revealed = app.reveal_note(&note.id, "1234").unwrap();
        assert_eq!(revealed.content, "my secret thoughts");
        assert!(app.recovery_status().unwrap().active_note_open);
    }

    #[test]
    fn reveal_with_wrong_note_pass_errors() {
        let mut app = secure();
        app.set_up_recovery("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "1234").unwrap();
        app.clear_active();

        assert!(matches!(
            app.reveal_note(&note.id, "wrong"),
            Err(SecureNotesError::Vault(VaultError::InvalidPassphrase))
        ));
        assert!(!app.recovery_status().unwrap().active_note_open);
    }

    #[test]
    fn two_notes_use_independent_passwords() {
        let mut app = secure();
        app.set_up_recovery("master-pass").unwrap();

        let note1 = app.create_note(input("One", "first body")).unwrap();
        let note2 = app.create_note(input("Two", "second body")).unwrap();

        app.protect_note(&note1.id, "1234").unwrap();
        app.protect_note(&note2.id, "321").unwrap();
        app.clear_active();

        // Each opens with its own password.
        assert_eq!(
            app.reveal_note(&note1.id, "1234").unwrap().content,
            "first body"
        );
        app.clear_active();
        assert_eq!(
            app.reveal_note(&note2.id, "321").unwrap().content,
            "second body"
        );
        app.clear_active();

        // And neither cross-unlocks the other.
        assert!(matches!(
            app.reveal_note(&note1.id, "321"),
            Err(SecureNotesError::Vault(VaultError::InvalidPassphrase))
        ));
        assert!(matches!(
            app.reveal_note(&note2.id, "1234"),
            Err(SecureNotesError::Vault(VaultError::InvalidPassphrase))
        ));
    }

    #[test]
    fn reveal_non_protected_note_returns_it_unchanged() {
        let mut app = secure();
        let note = app.create_note(input("Public", "plain body")).unwrap();

        let revealed = app.reveal_note(&note.id, "irrelevant").unwrap();
        assert!(!revealed.is_protected);
        assert_eq!(revealed.content, "plain body");
        assert!(!app.recovery_status().unwrap().active_note_open);
    }

    #[test]
    fn list_notes_always_blanks_protected_content_even_when_active() {
        let mut app = secure();
        app.set_up_recovery("master-pass").unwrap();
        let note = app.create_note(input("Diary", "plaintext-marker")).unwrap();
        app.protect_note(&note.id, "1234").unwrap();

        // The note is active right now, yet the list must still blank it.
        assert!(app.recovery_status().unwrap().active_note_open);
        let listed = app.list_notes().unwrap();
        let protected = listed.iter().find(|n| n.id == note.id).unwrap();
        assert!(protected.is_protected);
        assert_eq!(protected.content, "");
    }

    #[test]
    fn update_protected_note_requires_it_to_be_active() {
        let mut app = secure();
        app.set_up_recovery("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "1234").unwrap();
        app.clear_active();

        assert!(matches!(
            app.update_note(update(&note.id, "new secret")),
            Err(SecureNotesError::Vault(VaultError::Locked))
        ));
    }

    #[test]
    fn update_protected_note_reseals_when_active_and_roundtrips() {
        let mut app = secure();
        app.set_up_recovery("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "1234").unwrap();

        let updated = app.update_note(update(&note.id, "new secret")).unwrap();
        assert!(updated.is_protected);
        assert_eq!(updated.content, ""); // blanked on the way out

        // Revealing again with the SAME note password returns the new content.
        app.clear_active();
        let revealed = app.reveal_note(&note.id, "1234").unwrap();
        assert_eq!(revealed.content, "new secret");
    }

    #[test]
    fn unprotect_restores_plaintext_and_clears_flag() {
        let mut app = secure();
        app.set_up_recovery("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "1234").unwrap();

        let restored = app.unprotect_note(&note.id, "1234").unwrap();
        assert!(!restored.is_protected);
        assert_eq!(restored.content, "secret");
        assert!(!app.recovery_status().unwrap().active_note_open);

        let listed = app.list_notes().unwrap();
        let plain = listed.iter().find(|n| n.id == note.id).unwrap();
        assert!(!plain.is_protected);
        assert_eq!(plain.content, "secret");
    }

    #[test]
    fn unprotect_with_wrong_note_pass_errors() {
        let mut app = secure();
        app.set_up_recovery("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "1234").unwrap();

        assert!(matches!(
            app.unprotect_note(&note.id, "wrong"),
            Err(SecureNotesError::Vault(VaultError::InvalidPassphrase))
        ));
    }

    #[test]
    fn recover_note_restores_content_with_master_and_removes_old_pass() {
        let mut app = secure();
        app.set_up_recovery("master-pass").unwrap();
        let note = app.create_note(input("Diary", "forgotten secret")).unwrap();
        app.protect_note(&note.id, "note-pass").unwrap();
        app.clear_active();

        // The note password is forgotten; the master passphrase recovers it.
        let recovered = app.recover_note(&note.id, "master-pass").unwrap();
        assert!(!recovered.is_protected);
        assert_eq!(recovered.content, "forgotten secret");

        // The note is now plain: the old password is gone and content is visible.
        let listed = app.list_notes().unwrap();
        let plain = listed.iter().find(|n| n.id == note.id).unwrap();
        assert!(!plain.is_protected);
        assert_eq!(plain.content, "forgotten secret");
    }

    #[test]
    fn recover_note_with_wrong_master_pass_fails() {
        let mut app = secure();
        app.set_up_recovery("master-pass").unwrap();
        let note = app.create_note(input("Diary", "secret")).unwrap();
        app.protect_note(&note.id, "note-pass").unwrap();
        app.clear_active();

        assert!(matches!(
            app.recover_note(&note.id, "wrong-master"),
            Err(SecureNotesError::Vault(VaultError::InvalidPassphrase))
        ));
        // The note remains protected after a failed recovery.
        let listed = app.list_notes().unwrap();
        let still = listed.iter().find(|n| n.id == note.id).unwrap();
        assert!(still.is_protected);
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
        app.set_up_recovery("master-pass").unwrap();
        let note = app.create_note(input("Secret", "classified intel")).unwrap();
        app.protect_note(&note.id, "1234").unwrap();

        let (_, document) = app.export_markdown(&note.id).unwrap();
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
        app.set_up_recovery("master-pass").unwrap();
        let note = app.create_note(input("Journal", "uniquesecretword")).unwrap();
        app.protect_note(&note.id, "1234").unwrap();

        assert!(app.recovery_status().unwrap().active_note_open);
        assert!(app.search_notes("uniquesecretword").unwrap().is_empty());
    }
}
