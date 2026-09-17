use crate::ApplicationError;
use chrono::Utc;
use domain::{AppId, AppSettings, SubmissionRecord, SubmissionValue};
use serde::{Deserialize, Serialize};

pub trait AppRepository {
    fn load_settings(&self) -> Result<AppSettings, ApplicationError>;
    fn save_submission(&self, submission: SubmissionRecord) -> Result<(), ApplicationError>;
    fn list_submissions(&self) -> Result<Vec<SubmissionRecord>, ApplicationError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionPayload {
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct AppService<R> {
    repository: R,
}

impl<R> AppService<R>
where
    R: AppRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn submit(&self, payload: SubmissionPayload) -> Result<(), ApplicationError> {
        let settings = self.repository.load_settings()?;
        let value = SubmissionValue::new(payload.value)?;

        let record = SubmissionRecord {
            id: AppId::new(format!(
                "{}-{}",
                settings.app_name,
                Utc::now().timestamp_millis()
            ))?,
            title: "submit".to_string(),
            description: value.as_str().to_string(),
            created_at: Utc::now(),
            active: settings.enabled,
        };

        self.repository.save_submission(record)
    }

    pub fn list_records(&self) -> Result<Vec<SubmissionRecord>, ApplicationError> {
        self.repository.list_submissions()
    }
}
