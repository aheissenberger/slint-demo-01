use crate::{ApplicationCommand, ApplicationError};
use chrono::Utc;
use domain::{AppId, AppSettings, SubmissionRecord};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub trait AppRepository {
    fn load_settings(&self) -> Result<AppSettings, ApplicationError>;
    fn save_settings(&self, settings: &AppSettings) -> Result<(), ApplicationError>;
    fn list_records(&self) -> Result<Vec<SubmissionRecord>, ApplicationError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionPayload {
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionCommand {
    pub command: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct AppService<R> {
    repository: R,
    records: Arc<Mutex<Vec<SubmissionRecord>>>,
}

impl<R> AppService<R>
where
    R: AppRepository,
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            records: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn execute_command(&self, command: SubmissionCommand) -> Result<String, ApplicationError> {
        let parsed = ApplicationCommand::parse(&command.command)?;
        let settings = self.repository.load_settings()?;
        let value = command
            .arguments
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if value.trim().is_empty() {
            return Err(ApplicationError::InvalidPayload(
                "Wert darf nicht leer sein".into(),
            ));
        }

        let record = SubmissionRecord {
            id: AppId::new(format!(
                "{}-{}",
                settings.app_name,
                Utc::now().timestamp_millis()
            ))?,
            title: match parsed {
                ApplicationCommand::Submit => "submit".to_string(),
            },
            description: value.to_string(),
            created_at: Utc::now(),
            active: settings.enabled,
        };

        self.records
            .lock()
            .map_err(|_| ApplicationError::Repository("Datensatzsperre beschädigt".into()))?
            .push(record);
        Ok("Befehl ausgeführt".into())
    }

    pub fn list_records(&self) -> Result<Vec<SubmissionRecord>, ApplicationError> {
        self.records
            .lock()
            .map(|records| records.clone())
            .map_err(|_| ApplicationError::Repository("Datensatzsperre beschädigt".into()))
    }

    pub fn validate_input(&self, value: &str) -> Result<String, ApplicationError> {
        if value.trim().is_empty() {
            return Err(ApplicationError::InvalidPayload("Feld erforderlich".into()));
        }

        Ok(value.trim().to_string())
    }
}
