use crate::domain::note::{Note, NoteError};
use crate::ports::note_repository::NoteRepository;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;

pub struct SqliteNoteRepository {
    connection: Connection,
}

impl SqliteNoteRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, NoteError> {
        let connection = Connection::open(path).map_err(storage_error)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(storage_error)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(storage_error)?;

        Self::from_connection(connection)
    }

    fn from_connection(connection: Connection) -> Result<Self, NoteError> {
        let repository = Self { connection };
        repository.migrate()?;
        Ok(repository)
    }

    fn migrate(&self) -> Result<(), NoteError> {
        self.connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS notes (
                    id TEXT PRIMARY KEY NOT NULL,
                    title TEXT NOT NULL,
                    content TEXT NOT NULL,
                    category TEXT NOT NULL,
                    tags TEXT NOT NULL,
                    color TEXT NOT NULL,
                    is_favorite INTEGER NOT NULL,
                    is_archived INTEGER NOT NULL,
                    is_protected INTEGER NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );",
            )
            .map_err(storage_error)
    }
}

impl NoteRepository for SqliteNoteRepository {
    fn insert(&mut self, note: Note) -> Result<Note, NoteError> {
        write_note(&self.connection, &note, "INSERT INTO notes")?;
        Ok(note)
    }

    fn list(&self) -> Result<Vec<Note>, NoteError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, title, content, category, tags, color, is_favorite, is_archived,
                    is_protected, created_at, updated_at
                FROM notes
                ORDER BY CAST(updated_at AS INTEGER) DESC",
            )
            .map_err(storage_error)?;
        let notes = statement
            .query_map([], row_to_note)
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)?;

        Ok(notes)
    }

    fn find(&self, id: &str) -> Result<Option<Note>, NoteError> {
        self.connection
            .query_row(
                "SELECT id, title, content, category, tags, color, is_favorite, is_archived,
                    is_protected, created_at, updated_at
                FROM notes
                WHERE id = ?1",
                params![id],
                row_to_note,
            )
            .optional()
            .map_err(storage_error)
    }

    fn replace(&mut self, note: Note) -> Result<Note, NoteError> {
        write_note(&self.connection, &note, "REPLACE INTO notes")?;
        Ok(note)
    }

    fn delete(&mut self, id: &str) -> Result<bool, NoteError> {
        let changed = self
            .connection
            .execute("DELETE FROM notes WHERE id = ?1", params![id])
            .map_err(storage_error)?;

        Ok(changed > 0)
    }
}

fn write_note(connection: &Connection, note: &Note, prefix: &str) -> Result<(), NoteError> {
    let tags = serde_json::to_string(&note.tags).map_err(storage_error)?;
    let sql = format!(
        "{prefix} (
            id, title, content, category, tags, color, is_favorite, is_archived, is_protected,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
    );

    connection
        .execute(
            &sql,
            params![
                note.id,
                note.title,
                note.content,
                note.category,
                tags,
                note.color,
                note.is_favorite,
                note.is_archived,
                note.is_protected,
                note.created_at,
                note.updated_at,
            ],
        )
        .map_err(storage_error)?;

    Ok(())
}

fn row_to_note(row: &Row<'_>) -> rusqlite::Result<Note> {
    let tags_json: String = row.get(4)?;
    let tags = serde_json::from_str(&tags_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })?;

    Ok(Note {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        category: row.get(3)?,
        tags,
        color: row.get(5)?,
        is_favorite: row.get(6)?,
        is_archived: row.get(7)?,
        is_protected: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn storage_error(error: impl std::fmt::Display) -> NoteError {
    NoteError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository() -> SqliteNoteRepository {
        SqliteNoteRepository::from_connection(Connection::open_in_memory().expect("db should open"))
            .expect("migration should run")
    }

    fn note(id: &str, title: &str, updated_at: &str) -> Note {
        Note {
            id: id.to_string(),
            title: title.to_string(),
            content: "Body".to_string(),
            category: "Inbox".to_string(),
            tags: Vec::new(),
            color: "slate".to_string(),
            is_favorite: false,
            is_archived: false,
            is_protected: false,
            created_at: "100".to_string(),
            updated_at: updated_at.to_string(),
        }
    }

    #[test]
    fn migration_open_works() {
        let repository = repository();

        let count: i64 = repository
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'notes'",
                [],
                |row| row.get(0),
            )
            .expect("table query should succeed");

        assert_eq!(count, 1);
    }

    #[test]
    fn insert_find_list_replace_and_delete_round_trip() {
        let mut repository = repository();

        let inserted = repository
            .insert(note("note-1", "First", "100"))
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
        assert_eq!(repository.list().expect("list should succeed").len(), 1);

        let replaced = repository
            .replace(note("note-1", "Updated", "200"))
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
    fn tags_and_booleans_persist() {
        let mut repository = repository();
        let mut original = note("note-1", "First", "100");
        original.tags = vec!["Rust".to_string(), "SQLite".to_string()];
        original.is_favorite = true;
        original.is_archived = true;
        original.is_protected = true;

        repository.insert(original).expect("insert should succeed");

        let persisted = repository
            .find("note-1")
            .expect("find should succeed")
            .expect("note should exist");
        assert_eq!(persisted.tags, vec!["Rust", "SQLite"]);
        assert!(persisted.is_favorite);
        assert!(persisted.is_archived);
        assert!(persisted.is_protected);
    }

    fn unique_db_path() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "slate-persist-{}-{}.sqlite",
            std::process::id(),
            nonce
        ))
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
    }

    #[test]
    fn note_survives_close_and_reopen_on_disk() {
        let path = unique_db_path();
        cleanup(&path);

        {
            let mut repository =
                SqliteNoteRepository::open(&path).expect("repository should open on disk");
            repository
                .insert(note("note-1", "Survives restart", "100"))
                .expect("insert should succeed");
        } // repository dropped here -> simulates closing the app

        let reopened =
            SqliteNoteRepository::open(&path).expect("repository should reopen on disk");
        let found = reopened
            .find("note-1")
            .expect("find should succeed")
            .expect("note must survive close and reopen");
        assert_eq!(found.title, "Survives restart");

        cleanup(&path);
    }

    #[test]
    fn note_survives_reopen_with_second_connection_alive_on_close() {
        // Mirrors the real app: notes + vault repositories hold two connections
        // to the same file. The notes connection is dropped while the second
        // connection is still alive, so its close cannot run a full checkpoint.
        let path = unique_db_path();
        cleanup(&path);

        let sidecar = Connection::open(&path).expect("sidecar connection should open");

        {
            let mut repository =
                SqliteNoteRepository::open(&path).expect("repository should open on disk");
            repository
                .insert(note("note-1", "Survives restart", "100"))
                .expect("insert should succeed");
        } // notes connection dropped while `sidecar` is still open

        let reopened =
            SqliteNoteRepository::open(&path).expect("repository should reopen on disk");
        let found = reopened
            .find("note-1")
            .expect("find should succeed")
            .expect("note must survive close and reopen with a second connection alive");
        assert_eq!(found.title, "Survives restart");

        drop(sidecar);
        cleanup(&path);
    }

    #[test]
    fn list_orders_by_descending_updated_at() {
        let mut repository = repository();
        repository
            .insert(note("note-1", "Oldest", "9"))
            .expect("insert should succeed");
        repository
            .insert(note("note-2", "Newest", "10"))
            .expect("insert should succeed");
        repository
            .insert(note("note-3", "Middle", "8"))
            .expect("insert should succeed");

        let ids = repository
            .list()
            .expect("list should succeed")
            .into_iter()
            .map(|note| note.id)
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["note-2", "note-1", "note-3"]);
    }
}
