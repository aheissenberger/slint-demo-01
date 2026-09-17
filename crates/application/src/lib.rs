use chrono::Utc;
use domain::{AppId, AppStateSnapshot, DomainError, ExampleRecord, ExampleSettings};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub revision: u64,
    pub screen: String,
    pub input: String,
    pub development_file_path: String,
    pub selected_file: String,
    pub status: String,
    pub busy: bool,
    pub about_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppCommand {
    SetInput(String),
    SetDevelopmentFilePath(String),
    SelectDevelopmentFile,
    Reset,
    Submit,
    OpenAbout,
    CloseAbout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub message: String,
    pub snapshot: AppSnapshot,
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

#[derive(Clone)]
pub struct AppStateStore<R> {
    service: AppService<R>,
    state: Arc<Mutex<AppSnapshot>>,
    subscribers: Arc<Mutex<Vec<Sender<AppSnapshot>>>>,
}

impl<R> AppStateStore<R>
where
    R: ExampleRepository + Clone,
{
    pub fn new(service: AppService<R>) -> Self {
        Self {
            service,
            state: Arc::new(Mutex::new(AppSnapshot {
                revision: 0,
                screen: "main".to_string(),
                input: String::new(),
                development_file_path: "/workspace/Cargo.toml".to_string(),
                selected_file: String::new(),
                status: "bereit".to_string(),
                busy: false,
                about_open: false,
            })),
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn snapshot(&self) -> Result<AppSnapshot, ApplicationError> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| ApplicationError::Repository("Zustandssperre beschädigt".into()))
    }

    fn validate_development_file_path(path: &str) -> Result<String, ApplicationError> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err(ApplicationError::InvalidPayload(
                "Entwicklungsdateipfad darf nicht leer sein".into(),
            ));
        }
        if trimmed.len() > 4096 {
            return Err(ApplicationError::InvalidPayload(
                "Entwicklungsdateipfad überschreitet 4096 Zeichen".into(),
            ));
        }
        if trimmed.contains('\0') {
            return Err(ApplicationError::InvalidPayload(
                "Entwicklungsdateipfad enthält ungültige Nullbytes".into(),
            ));
        }

        let path = Path::new(trimmed);
        if path.is_dir() {
            return Err(ApplicationError::InvalidPayload(
                "Entwicklungsdateipfad muss auf eine Datei verweisen".into(),
            ));
        }

        Ok(trimmed.to_string())
    }

    pub fn subscribe(&self) -> Result<Receiver<AppSnapshot>, ApplicationError> {
        let (sender, receiver) = mpsc::channel();
        self.subscribers
            .lock()
            .map_err(|_| ApplicationError::Repository("Abonnementsperre beschädigt".into()))?
            .push(sender);
        Ok(receiver)
    }

    pub fn dispatch(&self, command: AppCommand) -> Result<CommandResult, ApplicationError> {
        let message = match command {
            AppCommand::SetInput(value) => {
                if value.len() > 4096 {
                    return Err(ApplicationError::InvalidPayload(
                        "Eingabe überschreitet 4096 Zeichen".into(),
                    ));
                }
                self.update(|state| state.input = value)?;
                "Wert aktualisiert".to_string()
            }
            AppCommand::SetDevelopmentFilePath(value) => {
                let path = Self::validate_development_file_path(&value)?;
                self.update(|state| state.development_file_path = path)?;
                "Entwicklungsdateipfad aktualisiert".to_string()
            }
            AppCommand::SelectDevelopmentFile => {
                let path =
                    Self::validate_development_file_path(&self.snapshot()?.development_file_path)?;
                self.update(|state| state.selected_file = path)?;
                "Datei ausgewählt".to_string()
            }
            AppCommand::Reset => {
                self.update(|state| {
                    state.input.clear();
                    state.status = "bereit".to_string();
                })?;
                "zurückgesetzt".to_string()
            }
            AppCommand::OpenAbout => {
                self.update(|state| state.about_open = true)?;
                "Info-Dialog geöffnet".to_string()
            }
            AppCommand::CloseAbout => {
                self.update(|state| state.about_open = false)?;
                "Info-Dialog geschlossen".to_string()
            }
            AppCommand::Submit => {
                let value = self.service.validate_input(&self.snapshot()?.input)?;
                self.update(|state| {
                    state.busy = true;
                    state.status = "wird ausgeführt".to_string();
                })?;
                let result = self.service.execute_command(ExampleCommand {
                    command: "submit".to_string(),
                    arguments: serde_json::json!({ "value": value }),
                });
                match result {
                    Ok(message) => {
                        self.update(|state| {
                            state.busy = false;
                            state.status = "erfolgreich".to_string();
                        })?;
                        message
                    }
                    Err(error) => {
                        self.update(|state| {
                            state.busy = false;
                            state.status = "Fehler".to_string();
                        })?;
                        return Err(error);
                    }
                }
            }
        };

        Ok(CommandResult {
            message,
            snapshot: self.snapshot()?,
        })
    }

    pub fn service(&self) -> &AppService<R> {
        &self.service
    }

    fn update<F>(&self, update: F) -> Result<(), ApplicationError>
    where
        F: FnOnce(&mut AppSnapshot),
    {
        let snapshot = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| ApplicationError::Repository("Zustandssperre beschädigt".into()))?;
            let previous = state.clone();
            update(&mut state);
            if *state == previous {
                return Ok(());
            }
            state.revision = state.revision.wrapping_add(1);
            state.clone()
        };
        let mut subscribers = self
            .subscribers
            .lock()
            .map_err(|_| ApplicationError::Repository("Abonnementsperre beschädigt".into()))?;
        subscribers.retain(|subscriber| subscriber.send(snapshot.clone()).is_ok());
        Ok(())
    }
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

    #[test]
    fn state_store_is_shared_by_commands_and_subscribers() {
        let store = AppStateStore::new(AppService::new(FakeRepository));
        let updates = store.subscribe().unwrap();

        store.dispatch(AppCommand::SetInput("Test".into())).unwrap();

        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.input, "Test");
        assert_eq!(updates.recv().unwrap(), snapshot);
    }

    #[test]
    fn state_store_rejects_submit_without_input() {
        let store = AppStateStore::new(AppService::new(FakeRepository));
        let error = store.dispatch(AppCommand::Submit).unwrap_err();
        assert!(error.to_string().contains("Feld erforderlich"));
        assert!(!store.snapshot().unwrap().busy);
    }

    #[test]
    fn state_store_does_not_publish_unchanged_state() {
        let store = AppStateStore::new(AppService::new(FakeRepository));
        let updates = store.subscribe().unwrap();

        store.dispatch(AppCommand::OpenAbout).unwrap();
        updates.recv().unwrap();
        let revision = store.snapshot().unwrap().revision;

        store.dispatch(AppCommand::OpenAbout).unwrap();

        assert!(updates.try_recv().is_err());
        assert_eq!(store.snapshot().unwrap().revision, revision);
    }

    #[test]
    fn state_store_rejects_directory_paths_for_the_dev_file_picker() {
        let store = AppStateStore::new(AppService::new(FakeRepository));
        let error = store
            .dispatch(AppCommand::SetDevelopmentFilePath("/workspace".into()))
            .unwrap_err();
        assert!(error.to_string().contains("muss auf eine Datei verweisen"));
        assert_eq!(
            store.snapshot().unwrap().development_file_path,
            "/workspace/Cargo.toml"
        );
    }
}
