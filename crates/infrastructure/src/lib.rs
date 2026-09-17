use application::{AppService, ApplicationError, ExampleRepository};
use domain::{ExampleRecord, ExampleSettings};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryRepository {
    settings: ExampleSettings,
    records: Vec<ExampleRecord>,
}

impl ExampleRepository for MemoryRepository {
    fn load_settings(&self) -> Result<ExampleSettings, ApplicationError> {
        Ok(self.settings.clone())
    }

    fn save_settings(&self, settings: &ExampleSettings) -> Result<(), ApplicationError> {
        let _ = settings;
        Ok(())
    }

    fn list_records(&self) -> Result<Vec<ExampleRecord>, ApplicationError> {
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
