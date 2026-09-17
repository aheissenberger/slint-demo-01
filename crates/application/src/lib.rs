mod reducer;
mod use_cases;

pub use reducer::{
    AppAction, AppReducer, AppState, ApplicationCommand, ApplicationError, CommandResult,
};
pub use use_cases::{AppRepository, AppService, SubmissionCommand, SubmissionPayload};

use domain::AppStatus;
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};

#[derive(Clone)]
pub struct AppStateStore<R> {
    service: AppService<R>,
    state: Arc<Mutex<AppState>>,
    subscribers: Arc<Mutex<Vec<Sender<AppState>>>>,
}

impl<R> AppStateStore<R>
where
    R: AppRepository + Clone,
{
    pub fn new(service: AppService<R>) -> Self {
        Self {
            service,
            state: Arc::new(Mutex::new(AppState::new())),
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn current_state(&self) -> Result<AppState, ApplicationError> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| ApplicationError::Repository("Zustandssperre beschädigt".into()))
    }

    pub fn subscribe(&self) -> Result<Receiver<AppState>, ApplicationError> {
        let (sender, receiver) = mpsc::channel();
        self.subscribers
            .lock()
            .map_err(|_| ApplicationError::Repository("Abonnementsperre beschädigt".into()))?
            .push(sender);
        Ok(receiver)
    }

    pub fn dispatch(&self, action: AppAction) -> Result<CommandResult, ApplicationError> {
        match action {
            AppAction::Submit => self.execute_submit(),
            action => {
                let message = self.update(|state| AppReducer::apply(state, &action))?;
                Ok(CommandResult {
                    message,
                    state: self.current_state()?,
                })
            }
        }
    }

    pub fn service(&self) -> &AppService<R> {
        &self.service
    }

    fn execute_submit(&self) -> Result<CommandResult, ApplicationError> {
        let value = self.service.validate_input(&self.current_state()?.input)?;
        self.update(|state| {
            state.busy = true;
            state.status = AppStatus::Busy;
            Ok("wird ausgeführt".to_string())
        })?;

        let message = match self.service.execute_command(SubmissionCommand {
            command: "submit".to_string(),
            arguments: serde_json::json!({ "value": value }),
        }) {
            Ok(message) => {
                self.update(|state| {
                    state.busy = false;
                    state.status = AppStatus::Success;
                    Ok(message.clone())
                })?;
                message
            }
            Err(error) => {
                self.update(|state| {
                    state.busy = false;
                    state.status = AppStatus::Error;
                    Ok("Fehler".to_string())
                })?;
                return Err(error);
            }
        };

        Ok(CommandResult {
            message,
            state: self.current_state()?,
        })
    }

    fn update<T, F>(&self, update: F) -> Result<T, ApplicationError>
    where
        F: FnOnce(&mut AppState) -> Result<T, ApplicationError>,
    {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Repository("Zustandssperre beschädigt".into()))?;
        let previous = state.clone();
        let result = update(&mut state)?;
        if *state == previous {
            return Ok(result);
        }

        state.revision = state.revision.wrapping_add(1);
        let updated_state = state.clone();
        drop(state);

        let mut subscribers = self
            .subscribers
            .lock()
            .map_err(|_| ApplicationError::Repository("Abonnementsperre beschädigt".into()))?;
        subscribers.retain(|subscriber| subscriber.send(updated_state.clone()).is_ok());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{AppSettings, SubmissionRecord};

    #[derive(Clone)]
    struct FakeRepository;

    impl AppRepository for FakeRepository {
        fn load_settings(&self) -> Result<AppSettings, ApplicationError> {
            Ok(AppSettings::default())
        }

        fn save_settings(&self, _settings: &AppSettings) -> Result<(), ApplicationError> {
            Ok(())
        }

        fn list_records(&self) -> Result<Vec<SubmissionRecord>, ApplicationError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn submit_command_persists_a_record_in_application_state() {
        let service = AppService::new(FakeRepository);
        service
            .execute_command(SubmissionCommand {
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

        store
            .dispatch(AppAction::SetInput {
                value: "Test".into(),
            })
            .unwrap();

        let state = store.current_state().unwrap();
        assert_eq!(state.input, "Test");
        assert_eq!(updates.recv().unwrap(), state);
    }

    #[test]
    fn state_store_rejects_submit_without_input() {
        let store = AppStateStore::new(AppService::new(FakeRepository));
        let error = store.dispatch(AppAction::Submit).unwrap_err();
        assert!(error.to_string().contains("Feld erforderlich"));
        assert!(!store.current_state().unwrap().busy);
    }

    #[test]
    fn state_store_does_not_publish_unchanged_state() {
        let store = AppStateStore::new(AppService::new(FakeRepository));
        let updates = store.subscribe().unwrap();

        store.dispatch(AppAction::OpenAbout).unwrap();
        updates.recv().unwrap();
        let revision = store.current_state().unwrap().revision;

        store.dispatch(AppAction::OpenAbout).unwrap();

        assert!(updates.try_recv().is_err());
        assert_eq!(store.current_state().unwrap().revision, revision);
    }

    #[test]
    fn state_store_rejects_directory_paths_for_the_dev_file_picker() {
        let store = AppStateStore::new(AppService::new(FakeRepository));
        let error = store
            .dispatch(AppAction::SetDevelopmentFilePath {
                path: "/workspace".into(),
            })
            .unwrap_err();
        assert!(error.to_string().contains("muss auf eine Datei verweisen"));
        assert_eq!(
            store.current_state().unwrap().development_file_path,
            "/workspace/Cargo.toml"
        );
    }
}
