use application::{AppRepository, AppService, ApplicationError};
use domain::{AppSettings, SubmissionRecord};
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

#[instrument(name = "infrastructure.initialized", fields(component = "repository"))]
pub fn initialize_repository() -> MemoryRepository {
    debug!("initializing default in-memory repository");
    MemoryRepository::with_default_settings()
}
