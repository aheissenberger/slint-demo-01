use application::{AppRepository, AppService, ApplicationError};
use domain::{AppSettings, SubmissionRecord};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryRepository {
    settings: AppSettings,
    records: Vec<SubmissionRecord>,
}

impl AppRepository for MemoryRepository {
    fn load_settings(&self) -> Result<AppSettings, ApplicationError> {
        Ok(self.settings.clone())
    }

    fn save_settings(&self, settings: &AppSettings) -> Result<(), ApplicationError> {
        let _ = settings;
        Ok(())
    }

    fn list_records(&self) -> Result<Vec<SubmissionRecord>, ApplicationError> {
        Ok(self.records.clone())
    }
}

impl MemoryRepository {
    pub fn with_default_settings() -> Self {
        Self::default()
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
