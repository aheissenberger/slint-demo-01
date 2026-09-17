mod file_repository;

pub use file_repository::FileRepository;

use application::{AppRepository, AppService, ApplicationError, NoteRepository};
use domain::{AppSettings, Note, NoteId, SubmissionRecord};
use std::sync::{Arc, Mutex};
use tracing::{debug, instrument};

#[derive(Debug, Clone, Default)]
pub struct MemoryRepository {
    state: Arc<Mutex<MemoryRepositoryState>>,
}

#[derive(Debug, Default)]
struct MemoryRepositoryState {
    settings: AppSettings,
    records: Vec<SubmissionRecord>,
    notes: Vec<Note>,
}

impl AppRepository for MemoryRepository {
    fn load_settings(&self) -> Result<AppSettings, ApplicationError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| ApplicationError::Repository("Repositorysperre beschädigt".into()))?
            .settings
            .clone())
    }

    fn save_settings(&self, settings: AppSettings) -> Result<(), ApplicationError> {
        self.state
            .lock()
            .map_err(|_| ApplicationError::Repository("Repositorysperre beschädigt".into()))?
            .settings = settings;
        Ok(())
    }

    fn save_submission(&self, submission: SubmissionRecord) -> Result<(), ApplicationError> {
        self.state
            .lock()
            .map_err(|_| ApplicationError::Repository("Repositorysperre beschädigt".into()))?
            .records
            .push(submission);
        Ok(())
    }

    fn list_submissions(&self) -> Result<Vec<SubmissionRecord>, ApplicationError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| ApplicationError::Repository("Repositorysperre beschädigt".into()))?
            .records
            .clone())
    }
}

impl NoteRepository for MemoryRepository {
    fn create_note(&self, note: Note) -> Result<(), ApplicationError> {
        self.state
            .lock()
            .map_err(|_| ApplicationError::Repository("Repositorysperre beschädigt".into()))?
            .notes
            .push(note);
        Ok(())
    }

    fn list_notes(&self) -> Result<Vec<Note>, ApplicationError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| ApplicationError::Repository("Repositorysperre beschädigt".into()))?
            .notes
            .clone())
    }

    fn update_note(&self, note: Note) -> Result<(), ApplicationError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Repository("Repositorysperre beschädigt".into()))?;
        if let Some(existing) = state
            .notes
            .iter_mut()
            .find(|existing| existing.id() == note.id())
        {
            *existing = note;
        }
        Ok(())
    }

    fn delete_note(&self, id: &NoteId) -> Result<(), ApplicationError> {
        self.state
            .lock()
            .map_err(|_| ApplicationError::Repository("Repositorysperre beschädigt".into()))?
            .notes
            .retain(|note| note.id() != id);
        Ok(())
    }
}

impl MemoryRepository {
    pub fn with_default_settings() -> Self {
        Self {
            state: Arc::new(Mutex::new(MemoryRepositoryState::default())),
        }
    }

    pub fn build_service(&self) -> AppService<Self> {
        AppService::new(self.clone())
    }
}

/// Builds the default production repository: a persistent, file-backed
/// adapter located in the platform data directory (overridable via the
/// `SLINT_DEMO_DATA_DIR` environment variable). Kept as the historical entry
/// point name so callers do not need to change beyond their type annotation.
#[instrument(name = "infrastructure.initialized", fields(component = "repository"))]
pub fn initialize_repository() -> FileRepository {
    debug!("initializing persistent file-backed repository");
    file_repository::initialize_file_repository()
}
