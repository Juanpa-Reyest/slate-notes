use crate::domain::note::{CreateNoteInput, Note, NoteError, UpdateNoteInput};
use crate::ports::note_repository::NoteRepository;

pub struct NotesService<R: NoteRepository> {
    next_id: u64,
    repository: R,
}

impl<R: NoteRepository> NotesService<R> {
    pub fn new(repository: R) -> Result<Self, NoteError> {
        let next_id = next_note_id(&repository.list()?);

        Ok(Self {
            next_id,
            repository,
        })
    }

    pub fn seed(&mut self) -> Result<(), NoteError> {
        if !self.list_notes()?.is_empty() {
            return Ok(());
        }

        self.create_note(CreateNoteInput {
            title: "Capture from the launcher".to_string(),
            content: "# Fast capture\n\nUse the centered search box to find notes or create a fresh Markdown note.".to_string(),
            category: Some("Inbox".to_string()),
        })?;

        let favorite = self.create_note(CreateNoteInput {
            title: "Architecture boundary".to_string(),
            content: "Commands call application use cases. The in-memory repository is only an adapter and can be replaced by SQLite later.".to_string(),
            category: Some("Design".to_string()),
        })?;
        self.toggle_favorite(&favorite.id)?;

        let archived = self.create_note(CreateNoteInput {
            title: "Protected notes placeholder".to_string(),
            content: "Encryption is intentionally not implemented in this prototype. The UI shows the protected state placeholder only.".to_string(),
            category: Some("Security".to_string()),
        })?;
        self.archive_note(&archived.id)?;

        Ok(())
    }

    pub fn create_note(&mut self, input: CreateNoteInput) -> Result<Note, NoteError> {
        let id = loop {
            let id = format!("note-{}", self.next_id);
            self.next_id += 1;

            if self.repository.find(&id)?.is_none() {
                break id;
            }
        };

        let note = Note::new(id, input)?;

        self.repository.insert(note)
    }

    pub fn list_notes(&self) -> Result<Vec<Note>, NoteError> {
        self.repository.list()
    }

    pub fn search_notes(&self, query: &str) -> Result<Vec<Note>, NoteError> {
        Ok(self
            .repository
            .list()?
            .into_iter()
            .filter(|note| note.matches_query(query))
            .collect())
    }

    pub fn update_note(&mut self, input: UpdateNoteInput) -> Result<Note, NoteError> {
        let mut note = self
            .repository
            .find(&input.id)?
            .ok_or(NoteError::NotFound)?;
        note.update(input)?;

        self.repository.replace(note)
    }

    pub fn toggle_favorite(&mut self, id: &str) -> Result<Note, NoteError> {
        let mut note = self.repository.find(id)?.ok_or(NoteError::NotFound)?;
        note.is_favorite = !note.is_favorite;
        note.touch();

        self.repository.replace(note)
    }

    pub fn archive_note(&mut self, id: &str) -> Result<Note, NoteError> {
        let mut note = self.repository.find(id)?.ok_or(NoteError::NotFound)?;
        note.is_archived = !note.is_archived;
        note.touch();

        self.repository.replace(note)
    }

    pub fn delete_note(&mut self, id: &str) -> Result<(), NoteError> {
        if self.repository.delete(id)? {
            return Ok(());
        }

        Err(NoteError::NotFound)
    }

    /// Fetch a single note by id.
    pub fn get_note(&self, id: &str) -> Result<Note, NoteError> {
        self.repository.find(id)?.ok_or(NoteError::NotFound)
    }

    /// Persist a note's protection flag together with its (already-transformed)
    /// content. Crypto stays out of here: the caller supplies sealed or plaintext
    /// content; this method only stores it.
    pub fn set_protection(
        &mut self,
        id: &str,
        is_protected: bool,
        content: String,
    ) -> Result<Note, NoteError> {
        let mut note = self.repository.find(id)?.ok_or(NoteError::NotFound)?;
        note.is_protected = is_protected;
        note.content = content;
        note.touch();
        self.repository.replace(note)
    }
}

fn next_note_id(notes: &[Note]) -> u64 {
    notes
        .iter()
        .filter_map(|note| note.id.strip_prefix("note-")?.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::memory_note_repository::MemoryNoteRepository;

    fn service() -> NotesService<MemoryNoteRepository> {
        NotesService::new(MemoryNoteRepository::default()).expect("service should initialize")
    }

    fn create_input(title: &str, content: &str, category: Option<&str>) -> CreateNoteInput {
        CreateNoteInput {
            title: title.to_string(),
            content: content.to_string(),
            category: category.map(str::to_string),
        }
    }

    fn update_input(id: &str) -> UpdateNoteInput {
        UpdateNoteInput {
            id: id.to_string(),
            title: "Updated".to_string(),
            content: "Body".to_string(),
            category: "Inbox".to_string(),
            tags: Vec::new(),
            color: "slate".to_string(),
        }
    }

    #[test]
    fn create_generates_sequential_note_ids() {
        let mut service = service();

        let first = service
            .create_note(create_input("First", "", None))
            .expect("note should be valid");
        let second = service
            .create_note(create_input("Second", "", None))
            .expect("note should be valid");

        assert_eq!(first.id, "note-1");
        assert_eq!(second.id, "note-2");
    }

    #[test]
    fn create_propagates_empty_note() {
        let mut service = service();

        let result = service.create_note(create_input("  ", "\n\t", None));

        assert!(matches!(result, Err(NoteError::EmptyNote)));
    }

    #[test]
    fn missing_ids_return_not_found_for_mutations() {
        let mut service = service();

        assert!(matches!(
            service.update_note(update_input("missing")),
            Err(NoteError::NotFound)
        ));
        assert!(matches!(
            service.delete_note("missing"),
            Err(NoteError::NotFound)
        ));
        assert!(matches!(
            service.toggle_favorite("missing"),
            Err(NoteError::NotFound)
        ));
        assert!(matches!(
            service.archive_note("missing"),
            Err(NoteError::NotFound)
        ));
    }

    #[test]
    fn search_returns_matches_and_excludes_non_matches() {
        let mut service = service();
        service
            .create_note(create_input(
                "Rust backend",
                "Domain tests",
                Some("Engineering"),
            ))
            .expect("note should be valid");
        service
            .create_note(create_input("Shopping", "Buy milk", Some("Personal")))
            .expect("note should be valid");

        let results = service
            .search_notes("backend")
            .expect("search should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust backend");
    }

    #[test]
    fn seed_is_idempotent_when_repository_already_has_notes() {
        let mut service = service();
        service
            .create_note(create_input("Existing", "", None))
            .expect("note should be valid");

        service.seed().expect("seed should succeed");

        let notes = service.list_notes().expect("list should succeed");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "Existing");
    }

    #[test]
    fn create_skips_existing_note_ids() {
        let mut repository = MemoryNoteRepository::default();
        repository
            .insert(Note {
                id: "note-1".to_string(),
                title: "Persisted".to_string(),
                content: String::new(),
                category: "Inbox".to_string(),
                tags: Vec::new(),
                color: "slate".to_string(),
                is_favorite: false,
                is_archived: false,
                is_protected: false,
                created_at: "1".to_string(),
                updated_at: "1".to_string(),
            })
            .expect("insert should succeed");
        let mut service = NotesService::new(repository).expect("service should initialize");

        let created = service
            .create_note(create_input("New", "", None))
            .expect("note should be valid");

        assert_eq!(created.id, "note-2");
    }

    #[test]
    fn create_starts_after_highest_existing_numeric_note_id() {
        let mut repository = MemoryNoteRepository::default();
        for id in ["note-999", "custom", "note-draft"] {
            repository
                .insert(Note {
                    id: id.to_string(),
                    title: "Persisted".to_string(),
                    content: String::new(),
                    category: "Inbox".to_string(),
                    tags: Vec::new(),
                    color: "slate".to_string(),
                    is_favorite: false,
                    is_archived: false,
                    is_protected: false,
                    created_at: "1".to_string(),
                    updated_at: "1".to_string(),
                })
                .expect("insert should succeed");
        }

        let mut service = NotesService::new(repository).expect("service should initialize");

        let created = service
            .create_note(create_input("New", "", None))
            .expect("note should be valid");

        assert_eq!(created.id, "note-1000");
    }
}
