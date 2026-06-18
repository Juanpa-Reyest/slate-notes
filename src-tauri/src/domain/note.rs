use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub category: String,
    pub tags: Vec<String>,
    pub color: String,
    pub is_favorite: bool,
    pub is_archived: bool,
    pub is_protected: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNoteInput {
    pub title: String,
    pub content: String,
    pub category: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNoteInput {
    pub id: String,
    pub title: String,
    pub content: String,
    pub category: String,
    pub tags: Vec<String>,
    pub color: String,
}

#[derive(Clone, Debug)]
pub enum NoteError {
    EmptyNote,
    NotFound,
    Storage(String),
}

impl std::fmt::Display for NoteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyNote => write!(formatter, "A note needs a title or content."),
            Self::NotFound => write!(formatter, "Note not found."),
            Self::Storage(message) => write!(formatter, "Storage error: {message}"),
        }
    }
}

impl Note {
    pub fn new(id: String, input: CreateNoteInput) -> Result<Self, NoteError> {
        validate_note_text(&input.title, &input.content)?;

        let now = timestamp();

        Ok(Self {
            id,
            title: input.title.trim().to_string(),
            content: input.content,
            category: input
                .category
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "Inbox".to_string()),
            tags: Vec::new(),
            color: "slate".to_string(),
            is_favorite: false,
            is_archived: false,
            is_protected: false,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn update(&mut self, input: UpdateNoteInput) -> Result<(), NoteError> {
        validate_note_text(&input.title, &input.content)?;

        self.title = input.title.trim().to_string();
        self.content = input.content;
        self.category = if input.category.trim().is_empty() {
            "Inbox".to_string()
        } else {
            input.category.trim().to_string()
        };
        self.tags = input.tags;
        self.color = input.color;
        self.updated_at = timestamp();

        Ok(())
    }

    pub fn matches_query(&self, query: &str) -> bool {
        if query.trim().is_empty() {
            return true;
        }

        let query = query.to_lowercase();

        self.title.to_lowercase().contains(&query)
            || self.content.to_lowercase().contains(&query)
            || self.category.to_lowercase().contains(&query)
            || self
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&query))
    }

    pub fn touch(&mut self) {
        self.updated_at = timestamp();
    }
}

fn validate_note_text(title: &str, content: &str) -> Result<(), NoteError> {
    if title.trim().is_empty() && content.trim().is_empty() {
        return Err(NoteError::EmptyNote);
    }

    Ok(())
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_input(title: &str, content: &str, category: Option<&str>) -> CreateNoteInput {
        CreateNoteInput {
            title: title.to_string(),
            content: content.to_string(),
            category: category.map(str::to_string),
        }
    }

    fn update_input(title: &str, content: &str, category: &str) -> UpdateNoteInput {
        UpdateNoteInput {
            id: "note-1".to_string(),
            title: title.to_string(),
            content: content.to_string(),
            category: category.to_string(),
            tags: vec!["Rust".to_string()],
            color: "amber".to_string(),
        }
    }

    #[test]
    fn new_rejects_blank_title_and_blank_content() {
        let result = Note::new("note-1".to_string(), create_input("  ", "\n\t", None));

        assert!(matches!(result, Err(NoteError::EmptyNote)));
    }

    #[test]
    fn new_trims_title() {
        let note = Note::new(
            "note-1".to_string(),
            create_input("  Title  ", "Body", None),
        )
        .expect("note should be valid");

        assert_eq!(note.title, "Title");
    }

    #[test]
    fn new_defaults_missing_or_empty_category_to_inbox() {
        let missing = Note::new("note-1".to_string(), create_input("Title", "", None))
            .expect("note should be valid");
        let empty = Note::new("note-2".to_string(), create_input("Title", "", Some("  ")))
            .expect("note should be valid");

        assert_eq!(missing.category, "Inbox");
        assert_eq!(empty.category, "Inbox");
    }

    #[test]
    fn new_initializes_default_color_booleans_and_tags() {
        let note = Note::new("note-1".to_string(), create_input("Title", "", None))
            .expect("note should be valid");

        assert_eq!(note.color, "slate");
        assert!(!note.is_favorite);
        assert!(!note.is_archived);
        assert!(!note.is_protected);
        assert!(note.tags.is_empty());
    }

    #[test]
    fn update_trims_title_and_category_and_defaults_blank_category() {
        let mut note = Note::new("note-1".to_string(), create_input("Title", "Body", None))
            .expect("note should be valid");

        note.update(update_input("  Updated  ", "Updated body", "  Design  "))
            .expect("update should be valid");
        assert_eq!(note.title, "Updated");
        assert_eq!(note.category, "Design");

        note.update(update_input("Updated", "Updated body", "  "))
            .expect("update should be valid");
        assert_eq!(note.category, "Inbox");
    }

    #[test]
    fn update_preserves_blank_title_and_blank_content_validation() {
        let mut note = Note::new("note-1".to_string(), create_input("Title", "Body", None))
            .expect("note should be valid");

        let result = note.update(update_input("  ", "\n\t", "Design"));

        assert!(matches!(result, Err(NoteError::EmptyNote)));
    }

    #[test]
    fn matches_query_is_case_insensitive_across_fields_and_empty_query_matches() {
        let mut note = Note::new(
            "note-1".to_string(),
            create_input("Rust Patterns", "Hexagonal architecture", Some("Design")),
        )
        .expect("note should be valid");
        note.tags = vec!["Backend".to_string(), "Testing".to_string()];

        assert!(note.matches_query(""));
        assert!(note.matches_query("rust"));
        assert!(note.matches_query("ARCHITECTURE"));
        assert!(note.matches_query("design"));
        assert!(note.matches_query("test"));
        assert!(!note.matches_query("frontend"));
    }
}
