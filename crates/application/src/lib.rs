use chrono::Utc;
use domain::{AppId, AppStateSnapshot, DomainError, ExampleRecord, ExampleSettings};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tracing::{debug, instrument};

pub trait ExampleRepository {
    fn load_settings(&self) -> Result<ExampleSettings, ApplicationError>;
    fn save_settings(&self, settings: &ExampleSettings) -> Result<(), ApplicationError>;
    fn list_records(&self) -> Result<Vec<ExampleRecord>, ApplicationError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleOperation {
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleCommand {
    pub command: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationCommand {
    Submit,
}

impl ApplicationCommand {
    pub fn parse(value: &str) -> Result<Self, ApplicationError> {
        match value {
            "submit" => Ok(Self::Submit),
            _ => Err(ApplicationError::InvalidPayload(format!(
                "nicht unterstützter Befehl: {value}"
            ))),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("Domänenfehler: {0}")]
    Domain(#[from] DomainError),
    #[error("Repository-Fehler: {0}")]
    Repository(String),
    #[error("ungültige Nutzdaten: {0}")]
    InvalidPayload(String),
}

#[derive(Debug, Clone)]
pub struct AppService<R> {
    repository: R,
    records: Arc<Mutex<Vec<ExampleRecord>>>,
}

impl<R> AppService<R>
where
    R: ExampleRepository,
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            records: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[instrument(name = "app.load_state", skip(self), fields(component = "app-service"))]
    pub fn load_state(&self) -> Result<AppStateSnapshot, ApplicationError> {
        debug!("loading application state");
        Ok(AppStateSnapshot::ready())
    }

    #[instrument(
        name = "app.execute_command",
        skip(self),
        fields(component = "app-service")
    )]
    pub fn execute_command(&self, command: ExampleCommand) -> Result<String, ApplicationError> {
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

        let record = ExampleRecord {
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

    pub fn list_records(&self) -> Result<Vec<ExampleRecord>, ApplicationError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{ExampleRecord, ExampleSettings};

    #[derive(Clone)]
    struct FakeRepository;

    impl ExampleRepository for FakeRepository {
        fn load_settings(&self) -> Result<ExampleSettings, ApplicationError> {
            Ok(ExampleSettings::default())
        }

        fn save_settings(&self, _settings: &ExampleSettings) -> Result<(), ApplicationError> {
            Ok(())
        }

        fn list_records(&self) -> Result<Vec<ExampleRecord>, ApplicationError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn submit_command_persists_a_record_in_application_state() {
        let service = AppService::new(FakeRepository);
        service
            .execute_command(ExampleCommand {
                command: "submit".into(),
                arguments: serde_json::json!({"value": "Test"}),
            })
            .unwrap();

        let records = service.list_records().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].description, "Test");
    }

    #[test]
    fn input_validation_trims_and_rejects_blank_values() {
        let service = AppService::new(FakeRepository);
        assert_eq!(service.validate_input("  Test ").unwrap(), "Test");
        assert!(matches!(
            service.validate_input("  "),
            Err(ApplicationError::InvalidPayload(_))
        ));
    }
}
