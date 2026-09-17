use crate::DomainError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Maximum length (in characters) allowed for a note title.
const NOTE_TITLE_MAX_LEN: usize = 200;
/// Maximum length (in characters) allowed for a note body.
const NOTE_BODY_MAX_LEN: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteId(String);

impl NoteId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::InvalidNoteId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteTitle(String);

impl NoteTitle {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into().trim().to_string();
        if value.is_empty() {
            return Err(DomainError::InvalidNoteTitle);
        }
        if value.chars().count() > NOTE_TITLE_MAX_LEN {
            return Err(DomainError::NoteTitleTooLong);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteBody(String);

impl NoteBody {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.chars().count() > NOTE_BODY_MAX_LEN {
            return Err(DomainError::NoteBodyTooLong);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A persisted user note, the first non-placeholder domain entity beyond the
/// generic submission demo flow. Notes carry their own lifecycle
/// (creation, rename, archival) independent of the UI submission callback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    id: NoteId,
    title: NoteTitle,
    body: NoteBody,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    archived: bool,
}

impl Note {
    pub fn new(id: NoteId, title: NoteTitle, body: NoteBody, created_at: DateTime<Utc>) -> Self {
        Self {
            id,
            title,
            body,
            created_at,
            updated_at: created_at,
            archived: false,
        }
    }

    pub fn from_persisted(
        id: NoteId,
        title: NoteTitle,
        body: NoteBody,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        archived: bool,
    ) -> Self {
        Self {
            id,
            title,
            body,
            created_at,
            updated_at,
            archived,
        }
    }

    pub fn id(&self) -> &NoteId {
        &self.id
    }

    pub fn title(&self) -> &NoteTitle {
        &self.title
    }

    pub fn body(&self) -> &NoteBody {
        &self.body
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn is_archived(&self) -> bool {
        self.archived
    }

    pub fn rename(&mut self, title: NoteTitle, at: DateTime<Utc>) {
        self.title = title;
        self.updated_at = at;
    }

    pub fn set_body(&mut self, body: NoteBody, at: DateTime<Utc>) {
        self.body = body;
        self.updated_at = at;
    }

    pub fn archive(&mut self, at: DateTime<Utc>) {
        self.archived = true;
        self.updated_at = at;
    }

    pub fn unarchive(&mut self, at: DateTime<Utc>) {
        self.archived = false;
        self.updated_at = at;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(seconds, 0).single().unwrap()
    }

    #[test]
    fn note_title_rejects_blank_and_overlong_values() {
        assert!(matches!(
            NoteTitle::new("   "),
            Err(DomainError::InvalidNoteTitle)
        ));
        assert!(matches!(
            NoteTitle::new("x".repeat(NOTE_TITLE_MAX_LEN + 1)),
            Err(DomainError::NoteTitleTooLong)
        ));
        assert_eq!(NoteTitle::new(" Einkaufen ").unwrap().as_str(), "Einkaufen");
    }

    #[test]
    fn note_body_rejects_overlong_values_but_allows_empty() {
        assert!(NoteBody::new("").is_ok());
        assert!(matches!(
            NoteBody::new("x".repeat(NOTE_BODY_MAX_LEN + 1)),
            Err(DomainError::NoteBodyTooLong)
        ));
    }

    #[test]
    fn note_lifecycle_tracks_updated_at_and_archival_state() {
        let id = NoteId::new("note-1").unwrap();
        let title = NoteTitle::new("Titel").unwrap();
        let body = NoteBody::new("Inhalt").unwrap();
        let mut note = Note::new(id, title, body, at(0));
        assert!(!note.is_archived());
        assert_eq!(note.updated_at(), at(0));

        note.archive(at(10));
        assert!(note.is_archived());
        assert_eq!(note.updated_at(), at(10));

        note.unarchive(at(20));
        assert!(!note.is_archived());
        assert_eq!(note.updated_at(), at(20));

        note.rename(NoteTitle::new("Neuer Titel").unwrap(), at(30));
        assert_eq!(note.title().as_str(), "Neuer Titel");
        assert_eq!(note.updated_at(), at(30));
    }
}
