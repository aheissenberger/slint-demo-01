use crate::ApplicationError;
use chrono::{DateTime, Utc};
use domain::{AppId, AppSettings, SubmissionRecord, SubmissionValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::{NoContext, Timestamp, Uuid};

pub trait AppRepository {
    fn load_settings(&self) -> Result<AppSettings, ApplicationError>;
    fn save_settings(&self, settings: AppSettings) -> Result<(), ApplicationError>;
    fn save_submission(&self, submission: SubmissionRecord) -> Result<(), ApplicationError>;
    fn list_submissions(&self) -> Result<Vec<SubmissionRecord>, ApplicationError>;
}

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub trait IdGenerator: Send + Sync {
    fn generate(&self, created_at: DateTime<Utc>) -> Result<AppId, ApplicationError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UuidV7Generator;

impl IdGenerator for UuidV7Generator {
    fn generate(&self, created_at: DateTime<Utc>) -> Result<AppId, ApplicationError> {
        let seconds = created_at.timestamp().try_into().map_err(|_| {
            ApplicationError::InvalidPayload(
                "Zeitpunkt für Übermittlungskennung liegt vor der Unix-Epoche".into(),
            )
        })?;
        let timestamp =
            Timestamp::from_unix(NoContext, seconds, created_at.timestamp_subsec_nanos());
        AppId::new(Uuid::new_v7(timestamp).to_string()).map_err(ApplicationError::from)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionPayload {
    pub value: String,
}

#[derive(Clone)]
pub struct AppService<R> {
    repository: R,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl<R> AppService<R>
where
    R: AppRepository,
{
    pub fn new(repository: R) -> Self {
        Self::with_dependencies(repository, SystemClock, UuidV7Generator)
    }

    pub fn with_dependencies(
        repository: R,
        clock: impl Clock + 'static,
        id_generator: impl IdGenerator + 'static,
    ) -> Self {
        Self {
            repository,
            clock: Arc::new(clock),
            id_generator: Arc::new(id_generator),
        }
    }

    pub fn submit(&self, payload: SubmissionPayload) -> Result<(), ApplicationError> {
        let settings = self.repository.load_settings()?;
        let value = SubmissionValue::new(payload.value)?;
        let created_at = self.clock.now();

        let record = SubmissionRecord {
            id: self.id_generator.generate(created_at)?,
            title: "submit".to_string(),
            description: value.as_str().to_string(),
            created_at,
            active: settings.enabled,
        };

        self.repository.save_submission(record)
    }

    pub fn list_records(&self) -> Result<Vec<SubmissionRecord>, ApplicationError> {
        self.repository.list_submissions()
    }

    pub fn load_settings(&self) -> Result<AppSettings, ApplicationError> {
        self.repository.load_settings()
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }

    /// Persists the given appearance preference, leaving every other
    /// persisted setting untouched.
    pub fn save_theme_mode(&self, theme_mode: &str) -> Result<(), ApplicationError> {
        let mut settings = self.repository.load_settings()?;
        settings.theme_mode = theme_mode.to_string();
        self.repository.save_settings(settings)
    }
}
