use crate::domain::note::{Note, NoteError};

pub trait NoteRepository {
    fn insert(&mut self, note: Note) -> Result<Note, NoteError>;
    fn list(&self) -> Result<Vec<Note>, NoteError>;
    fn find(&self, id: &str) -> Result<Option<Note>, NoteError>;
    fn replace(&mut self, note: Note) -> Result<Note, NoteError>;
    fn delete(&mut self, id: &str) -> Result<bool, NoteError>;
}
