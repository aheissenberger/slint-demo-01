use crate::ApplicationError;
use crate::{Clock, SystemClock};
use domain::{Note, NoteBody, NoteId, NoteTitle};
use std::sync::Arc;
use uuid::Uuid;

/// Persistence boundary for the [`Note`] entity. Kept separate from
/// [`crate::AppRepository`] so a repository can support one, both, or neither
/// without unrelated methods leaking across concerns.
pub trait NoteRepository {
    fn create_note(&self, note: Note) -> Result<(), ApplicationError>;
    fn list_notes(&self) -> Result<Vec<Note>, ApplicationError>;
    fn update_note(&self, note: Note) -> Result<(), ApplicationError>;
    fn delete_note(&self, id: &NoteId) -> Result<(), ApplicationError>;
}

/// Use cases for managing notes: a small but complete, meaningfully-named
/// domain workflow (create/rename/archive/delete) distinct from the generic
/// submission demo flow.
#[derive(Clone)]
pub struct NoteService<R> {
    repository: R,
    clock: Arc<dyn Clock>,
}

impl<R> NoteService<R>
where
    R: NoteRepository,
{
    pub fn new(repository: R) -> Self {
        Self::with_clock(repository, SystemClock)
    }

    pub fn with_clock(repository: R, clock: impl Clock + 'static) -> Self {
        Self {
            repository,
            clock: Arc::new(clock),
        }
    }

    pub fn create_note(
        &self,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Result<NoteId, ApplicationError> {
        let created_at = self.clock.now();
        let title = NoteTitle::new(title)?;
        let body = NoteBody::new(body)?;
        let id = NoteId::new(Uuid::now_v7().to_string())?;
        let note = Note::new(id.clone(), title, body, created_at);
        self.repository.create_note(note)?;
        Ok(id)
    }

    pub fn list_notes(&self) -> Result<Vec<Note>, ApplicationError> {
        self.repository.list_notes()
    }

    pub fn list_active_notes(&self) -> Result<Vec<Note>, ApplicationError> {
        Ok(self
            .list_notes()?
            .into_iter()
            .filter(|note| !note.is_archived())
            .collect())
    }

    pub fn rename_note(
        &self,
        id: &NoteId,
        new_title: impl Into<String>,
    ) -> Result<(), ApplicationError> {
        let mut note = self.find_note(id)?;
        note.rename(NoteTitle::new(new_title)?, self.clock.now());
        self.repository.update_note(note)
    }

    pub fn update_note(
        &self,
        id: &NoteId,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Result<(), ApplicationError> {
        let mut note = self.find_note(id)?;
        let now = self.clock.now();
        note.rename(NoteTitle::new(title)?, now);
        note.set_body(NoteBody::new(body)?, now);
        self.repository.update_note(note)
    }

    pub fn archive_note(&self, id: &NoteId) -> Result<(), ApplicationError> {
        let mut note = self.find_note(id)?;
        note.archive(self.clock.now());
        self.repository.update_note(note)
    }

    pub fn unarchive_note(&self, id: &NoteId) -> Result<(), ApplicationError> {
        let mut note = self.find_note(id)?;
        note.unarchive(self.clock.now());
        self.repository.update_note(note)
    }

    pub fn delete_note(&self, id: &NoteId) -> Result<(), ApplicationError> {
        self.repository.delete_note(id)
    }

    fn find_note(&self, id: &NoteId) -> Result<Note, ApplicationError> {
        self.list_notes()?
            .into_iter()
            .find(|note| note.id() == id)
            .ok_or_else(|| {
                ApplicationError::InvalidPayload(format!(
                    "Notiz {} wurde nicht gefunden",
                    id.as_str()
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone, Utc};
    use std::sync::Mutex;

    #[derive(Clone, Copy)]
    struct FixedClock(DateTime<Utc>);

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(seconds, 0).single().unwrap()
    }

    #[derive(Clone, Default)]
    struct FakeNoteRepository {
        notes: Arc<Mutex<Vec<Note>>>,
    }

    impl NoteRepository for FakeNoteRepository {
        fn create_note(&self, note: Note) -> Result<(), ApplicationError> {
            self.notes
                .lock()
                .map_err(|_| ApplicationError::Repository("Notizsperre beschädigt".into()))?
                .push(note);
            Ok(())
        }

        fn list_notes(&self) -> Result<Vec<Note>, ApplicationError> {
            self.notes
                .lock()
                .map(|notes| notes.clone())
                .map_err(|_| ApplicationError::Repository("Notizsperre beschädigt".into()))
        }

        fn update_note(&self, note: Note) -> Result<(), ApplicationError> {
            let mut notes = self
                .notes
                .lock()
                .map_err(|_| ApplicationError::Repository("Notizsperre beschädigt".into()))?;
            if let Some(existing) = notes.iter_mut().find(|existing| existing.id() == note.id()) {
                *existing = note;
            }
            Ok(())
        }

        fn delete_note(&self, id: &NoteId) -> Result<(), ApplicationError> {
            self.notes
                .lock()
                .map_err(|_| ApplicationError::Repository("Notizsperre beschädigt".into()))?
                .retain(|note| note.id() != id);
            Ok(())
        }
    }

    fn service(now: DateTime<Utc>) -> NoteService<FakeNoteRepository> {
        NoteService::with_clock(FakeNoteRepository::default(), FixedClock(now))
    }

    #[test]
    fn create_note_rejects_blank_titles() {
        let service = service(at(0));
        let result = service.create_note("   ", "Inhalt");
        assert!(result.is_err());
        assert!(service.list_notes().unwrap().is_empty());
    }

    #[test]
    fn create_list_rename_archive_and_delete_round_trip() {
        let service = service(at(0));
        let id = service.create_note("Einkaufen", "Milch, Brot").unwrap();

        let notes = service.list_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title().as_str(), "Einkaufen");
        assert!(!notes[0].is_archived());

        service.rename_note(&id, "Wocheneinkauf").unwrap();
        assert_eq!(
            service.list_notes().unwrap()[0].title().as_str(),
            "Wocheneinkauf"
        );

        service.archive_note(&id).unwrap();
        assert!(service.list_active_notes().unwrap().is_empty());
        assert_eq!(service.list_notes().unwrap().len(), 1);

        service.unarchive_note(&id).unwrap();
        assert_eq!(service.list_active_notes().unwrap().len(), 1);

        service.delete_note(&id).unwrap();
        assert!(service.list_notes().unwrap().is_empty());
    }

    #[test]
    fn operating_on_an_unknown_note_id_fails() {
        let service = service(at(0));
        let unknown = NoteId::new("does-not-exist").unwrap();
        assert!(service.rename_note(&unknown, "Neu").is_err());
        assert!(service.archive_note(&unknown).is_err());
    }

    #[test]
    fn deleting_an_unknown_note_id_is_idempotent() {
        let service = service(at(0));
        let unknown = NoteId::new("does-not-exist").unwrap();
        assert!(service.delete_note(&unknown).is_ok());
    }
}
