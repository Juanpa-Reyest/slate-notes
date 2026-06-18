use crate::domain::note::{Note, NoteError};
use crate::ports::note_repository::NoteRepository;
use std::collections::HashMap;

#[derive(Default)]
pub struct MemoryNoteRepository {
    notes: HashMap<String, Note>,
}

impl NoteRepository for MemoryNoteRepository {
    fn insert(&mut self, note: Note) -> Result<Note, NoteError> {
        self.notes.insert(note.id.clone(), note.clone());
        Ok(note)
    }

    fn list(&self) -> Result<Vec<Note>, NoteError> {
        let mut notes = self.notes.values().cloned().collect::<Vec<_>>();
        notes.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(notes)
    }

    fn find(&self, id: &str) -> Result<Option<Note>, NoteError> {
        Ok(self.notes.get(id).cloned())
    }

    fn replace(&mut self, note: Note) -> Result<Note, NoteError> {
        self.notes.insert(note.id.clone(), note.clone());
        Ok(note)
    }

    fn delete(&mut self, id: &str) -> Result<bool, NoteError> {
        Ok(self.notes.remove(id).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::note_repository::NoteRepository;

    fn note(id: &str, title: &str, updated_at: &str) -> Note {
        Note {
            id: id.to_string(),
            title: title.to_string(),
            content: String::new(),
            category: "Inbox".to_string(),
            tags: Vec::new(),
            color: "slate".to_string(),
            is_favorite: false,
            is_archived: false,
            is_protected: false,
            created_at: "1".to_string(),
            updated_at: updated_at.to_string(),
        }
    }

    #[test]
    fn insert_find_update_and_delete_notes() {
        let mut repository = MemoryNoteRepository::default();

        let inserted = repository
            .insert(note("note-1", "First", "1"))
            .expect("insert should succeed");
        assert_eq!(inserted.id, "note-1");
        assert_eq!(
            repository
                .find("note-1")
                .expect("find should succeed")
                .expect("note should exist")
                .title,
            "First"
        );

        let replaced = repository
            .replace(note("note-1", "Updated", "2"))
            .expect("replace should succeed");
        assert_eq!(replaced.title, "Updated");
        assert_eq!(
            repository
                .find("note-1")
                .expect("find should succeed")
                .expect("note should exist")
                .title,
            "Updated"
        );

        assert!(repository.delete("note-1").expect("delete should succeed"));
        assert!(repository
            .find("note-1")
            .expect("find should succeed")
            .is_none());
        assert!(!repository.delete("note-1").expect("delete should succeed"));
    }

    #[test]
    fn list_returns_notes_by_descending_updated_at() {
        let mut repository = MemoryNoteRepository::default();
        repository
            .insert(note("note-1", "Oldest", "100"))
            .expect("insert should succeed");
        repository
            .insert(note("note-2", "Newest", "300"))
            .expect("insert should succeed");
        repository
            .insert(note("note-3", "Middle", "200"))
            .expect("insert should succeed");

        let ids = repository
            .list()
            .expect("list should succeed")
            .into_iter()
            .map(|note| note.id)
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["note-2", "note-3", "note-1"]);
    }
}
