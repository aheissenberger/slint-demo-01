mod reducer;
mod use_cases;

pub use reducer::{
    ActiveDialog, AppAction, AppEffect, AppEvent, AppReducer, AppState, ApplicationError,
    CommandResult,
};
pub use use_cases::{AppRepository, AppService, SubmissionPayload};

use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateUpdate {
    pub state: AppState,
    pub effect: Option<AppEffect>,
}

#[derive(Clone)]
pub struct AppStateStore<R> {
    service: AppService<R>,
    state: Arc<Mutex<AppState>>,
    subscribers: Arc<Mutex<Vec<Sender<StateUpdate>>>>,
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

    pub fn subscribe(&self) -> Result<Receiver<StateUpdate>, ApplicationError> {
        let (sender, receiver) = mpsc::channel();
        self.subscribers
            .lock()
            .map_err(|_| ApplicationError::Repository("Abonnementsperre beschädigt".into()))?
            .push(sender);
        Ok(receiver)
    }

    pub fn dispatch(&self, action: AppAction) -> Result<CommandResult, ApplicationError> {
        self.reduce(action)
    }

    pub fn service(&self) -> &AppService<R> {
        &self.service
    }

    pub fn submit(&self) -> Result<CommandResult, ApplicationError> {
        let input = self.current_state()?.input;
        self.publish_event(AppEvent::SubmissionStarted)?;
        let result = self.service.submit(SubmissionPayload { value: input });
        let message = match result {
            Ok(()) => self.publish_event(AppEvent::SubmissionSucceeded)?,
            Err(error) => {
                self.publish_event(AppEvent::SubmissionFailed {
                    message: error.to_string(),
                })?;
                return Err(error);
            }
        };
        Ok(CommandResult {
            message,
            state: self.current_state()?,
        })
    }

    fn reduce(&self, action: AppAction) -> Result<CommandResult, ApplicationError> {
        let effect = match &action {
            AppAction::Focus { element_id } => Some(AppEffect::Focus {
                element_id: element_id.clone(),
            }),
            AppAction::RequestFilePicker => Some(AppEffect::OpenNativeFilePicker),
            _ => None,
        };
        let message = match self.update(effect, |state| AppReducer::apply(state, &action)) {
            Ok(message) => message,
            Err(error) => {
                self.publish_event(AppEvent::ActionFailed {
                    message: error.to_string(),
                })?;
                return Err(error);
            }
        };
        Ok(CommandResult {
            message,
            state: self.current_state()?,
        })
    }

    fn publish_event(&self, event: AppEvent) -> Result<String, ApplicationError> {
        self.update(None, |state| Ok(AppReducer::apply_event(state, &event)))
    }

    fn update<T, F>(&self, effect: Option<AppEffect>, update: F) -> Result<T, ApplicationError>
    where
        F: FnOnce(&mut AppState) -> Result<T, ApplicationError>,
    {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Repository("Zustandssperre beschädigt".into()))?;
        let previous = state.clone();
        let result = update(&mut state)?;
        if *state == previous && effect.is_none() {
            return Ok(result);
        }

        if *state != previous {
            state.revision = state.revision.wrapping_add(1);
        }
        let updated_state = state.clone();
        drop(state);

        let mut subscribers = self
            .subscribers
            .lock()
            .map_err(|_| ApplicationError::Repository("Abonnementsperre beschädigt".into()))?;
        let update = StateUpdate {
            state: updated_state,
            effect,
        };
        subscribers.retain(|subscriber| subscriber.send(update.clone()).is_ok());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{AppSettings, SubmissionRecord};

    #[derive(Clone, Default)]
    struct FakeRepository {
        records: Arc<Mutex<Vec<SubmissionRecord>>>,
    }

    impl AppRepository for FakeRepository {
        fn load_settings(&self) -> Result<AppSettings, ApplicationError> {
            Ok(AppSettings::default())
        }

        fn save_submission(&self, submission: SubmissionRecord) -> Result<(), ApplicationError> {
            self.records
                .lock()
                .map_err(|_| ApplicationError::Repository("Datensatzsperre beschädigt".into()))?
                .push(submission);
            Ok(())
        }

        fn list_submissions(&self) -> Result<Vec<SubmissionRecord>, ApplicationError> {
            self.records
                .lock()
                .map(|records| records.clone())
                .map_err(|_| ApplicationError::Repository("Datensatzsperre beschädigt".into()))
        }
    }

    #[test]
    fn submitting_persists_a_record_in_the_repository() {
        let service = AppService::new(FakeRepository::default());
        service
            .submit(SubmissionPayload {
                value: "Test".into(),
            })
            .unwrap();

        let records = service.list_records().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].description, "Test");
    }

    #[test]
    fn state_store_is_shared_by_commands_and_subscribers() {
        let store = AppStateStore::new(AppService::new(FakeRepository::default()));
        let updates = store.subscribe().unwrap();

        store
            .dispatch(AppAction::SetInput {
                value: "Test".into(),
            })
            .unwrap();

        let state = store.current_state().unwrap();
        assert_eq!(state.input, "Test");
        assert_eq!(updates.recv().unwrap().state, state);
    }

    #[test]
    fn submit_availability_uses_the_same_trimmed_input_rule_as_submission() {
        let store = AppStateStore::new(AppService::new(FakeRepository::default()));

        store
            .dispatch(AppAction::SetInput {
                value: "   ".into(),
            })
            .unwrap();
        assert!(!store.current_state().unwrap().can_submit());

        store
            .dispatch(AppAction::SetInput {
                value: "Test".into(),
            })
            .unwrap();
        assert!(store.current_state().unwrap().can_submit());
    }

    #[test]
    fn state_store_rejects_submit_without_input() {
        let store = AppStateStore::new(AppService::new(FakeRepository::default()));
        let error = store.submit().unwrap_err();
        assert!(error.to_string().contains("Wert darf nicht leer sein"));
        let state = store.current_state().unwrap();
        assert!(!state.busy);
        assert_eq!(state.status, domain::AppStatus::Error);
        assert_eq!(
            state.error_message.as_deref(),
            Some("Domänenfehler: Wert darf nicht leer sein")
        );
    }

    #[test]
    fn state_store_publishes_submission_lifecycle() {
        let store = AppStateStore::new(AppService::new(FakeRepository::default()));
        let updates = store.subscribe().unwrap();
        store
            .dispatch(AppAction::SetInput {
                value: "Test".into(),
            })
            .unwrap();
        updates.recv().unwrap();

        let result = store.submit().unwrap();
        assert_eq!(result.state.status, domain::AppStatus::Success);
        assert!(!result.state.busy);
        assert_eq!(
            updates.recv().unwrap().state.status,
            domain::AppStatus::Busy
        );
        assert_eq!(
            updates.recv().unwrap().state.status,
            domain::AppStatus::Success
        );
    }

    #[test]
    fn state_store_does_not_publish_unchanged_state() {
        let store = AppStateStore::new(AppService::new(FakeRepository::default()));
        let updates = store.subscribe().unwrap();

        store.dispatch(AppAction::OpenAbout).unwrap();
        updates.recv().unwrap();
        let revision = store.current_state().unwrap().revision;

        store.dispatch(AppAction::OpenAbout).unwrap();

        assert!(updates.try_recv().is_err());
        assert_eq!(store.current_state().unwrap().revision, revision);
    }

    #[test]
    fn focusing_a_control_publishes_a_transient_effect_without_mutating_state() {
        let store = AppStateStore::new(AppService::new(FakeRepository::default()));
        let updates = store.subscribe().unwrap();

        let result = store
            .dispatch(AppAction::Focus {
                element_id: "main.input".into(),
            })
            .unwrap();

        assert_eq!(result.message, "main.input fokussiert");
        assert_eq!(result.state, AppState::new());
        assert_eq!(
            updates.recv().unwrap().effect,
            Some(AppEffect::Focus {
                element_id: "main.input".into()
            })
        );
    }

    #[test]
    fn state_store_rejects_invalid_file_selection_paths() {
        let store = AppStateStore::new(AppService::new(FakeRepository::default()));
        let error = store
            .dispatch(AppAction::SelectFile { path: "\0".into() })
            .unwrap_err();
        assert!(error.to_string().contains("enthält ungültige Nullbytes"));
        assert_eq!(store.current_state().unwrap().selected_file, "");
    }

    #[test]
    fn file_picker_request_is_published_as_a_transient_effect() {
        let store = AppStateStore::new(AppService::new(FakeRepository::default()));
        let updates = store.subscribe().unwrap();

        store.dispatch(AppAction::RequestFilePicker).unwrap();
        assert_eq!(
            updates.recv().unwrap().effect,
            Some(AppEffect::OpenNativeFilePicker)
        );

        let revision = store.current_state().unwrap().revision;
        store.dispatch(AppAction::RequestFilePicker).unwrap();
        assert_eq!(
            updates.recv().unwrap().effect,
            Some(AppEffect::OpenNativeFilePicker)
        );
        assert_eq!(store.current_state().unwrap().revision, revision);

        store.dispatch(AppAction::CancelFileSelection).unwrap();
        assert!(updates.try_recv().is_err());
    }

    #[test]
    fn rejected_actions_are_reported_through_canonical_state() {
        let store = AppStateStore::new(AppService::new(FakeRepository::default()));

        let error = store
            .dispatch(AppAction::SelectFile { path: "\0".into() })
            .unwrap_err();

        let state = store.current_state().unwrap();
        assert_eq!(state.status, domain::AppStatus::Error);
        assert_eq!(
            state.error_message.as_deref(),
            Some(error.to_string().as_str())
        );
    }
}
