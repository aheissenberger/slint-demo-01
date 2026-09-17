mod localization;
mod notes;
mod reducer;
mod use_cases;

pub use localization::{Catalog, Locale};
pub use notes::{NoteRepository, NoteService};
pub use reducer::{
    ActiveDialog, AppAction, AppEffect, AppEvent, AppReducer, AppState, ApplicationError,
    CommandResult, ErrorCode, ErrorSeverity, FieldValidation, NoteListItem, TaskState, TaskStatus,
    ThemeMode, UiError,
};
pub use use_cases::{
    AppRepository, AppService, Clock, IdGenerator, SubmissionPayload, SystemClock, UuidV7Generator,
};

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};
use std::thread;

const MAX_PARALLEL_TASKS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateUpdate {
    pub state: AppState,
    pub effect: Option<AppEffect>,
}

#[derive(Clone)]
pub struct AppStateStore<R> {
    service: AppService<R>,
    note_service: NoteService<R>,
    publisher: StatePublisher,
    task_sequence: Arc<AtomicU64>,
    cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

#[derive(Clone)]
struct StatePublisher {
    state: Arc<Mutex<AppState>>,
    subscribers: Arc<Mutex<Vec<Sender<StateUpdate>>>>,
}

impl<R> AppStateStore<R>
where
    R: AppRepository + NoteRepository + Clone + Send + 'static,
{
    pub fn new(service: AppService<R>) -> Self {
        let mut initial_state = AppState::new();
        let note_service = NoteService::new(service.repository().clone());
        match service.load_settings() {
            Ok(settings) => match ThemeMode::parse(&settings.theme_mode) {
                Ok(mode) => initial_state.theme_mode = mode,
                Err(error) => {
                    tracing::warn!(%error, value = %settings.theme_mode, "unbekannter gespeicherter Darstellungsmodus, verwende Systemeinstellung");
                }
            },
            Err(error) => {
                tracing::warn!(%error, "gespeicherte Einstellungen konnten nicht geladen werden, verwende Vorgabewerte");
                initial_state.ui_error = Some(UiError::from_application_error(&error));
                if error.severity() == ErrorSeverity::Critical {
                    initial_state.active_dialog = Some(ActiveDialog::CriticalError);
                }
            }
        }
        match note_service.list_active_notes() {
            Ok(notes) => initial_state.replace_notes(notes.into_iter().map(Into::into).collect()),
            Err(error) => {
                tracing::warn!(%error, "gespeicherte Notizen konnten nicht geladen werden");
                initial_state.ui_error = Some(UiError::from_application_error(&error));
                if error.severity() == ErrorSeverity::Critical {
                    initial_state.active_dialog = Some(ActiveDialog::CriticalError);
                }
            }
        }
        let publisher = StatePublisher {
            state: Arc::new(Mutex::new(initial_state)),
            subscribers: Arc::new(Mutex::new(Vec::new())),
        };
        Self {
            service,
            note_service,
            publisher,
            task_sequence: Arc::new(AtomicU64::new(1)),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn current_state(&self) -> Result<AppState, ApplicationError> {
        self.publisher.current_state()
    }

    pub fn subscribe(&self) -> Result<Receiver<StateUpdate>, ApplicationError> {
        self.publisher.subscribe()
    }

    pub fn dispatch(&self, action: AppAction) -> Result<CommandResult, ApplicationError> {
        if matches!(action, AppAction::CancelSubmission) {
            return self.cancel_submission();
        }
        if matches!(action, AppAction::RetryLastFailedTask) {
            return self.retry_last_failed_submission();
        }
        self.reduce(action)
    }

    pub fn service(&self) -> &AppService<R> {
        &self.service
    }

    pub fn submit(&self) -> Result<CommandResult, ApplicationError> {
        self.start_submission(None)
    }

    pub fn cancel_submission(&self) -> Result<CommandResult, ApplicationError> {
        let state = self.current_state()?;
        let Some(task_id) = state.active_task_id else {
            return Ok(CommandResult {
                message: "Kein Vorgang aktiv".into(),
                state,
            });
        };
        self.cancel_task(&task_id)
    }

    pub fn cancel_task(&self, task_id: &str) -> Result<CommandResult, ApplicationError> {
        if let Some(cancellation) = self
            .cancellations
            .lock()
            .map_err(|_| ApplicationError::Repository("Abbruchsperre beschädigt".into()))?
            .get(task_id)
        {
            cancellation.store(true, Ordering::Release);
        }
        self.publisher.publish_event(AppEvent::SubmissionCancelled {
            task_id: task_id.to_string(),
        })
    }

    pub fn retry_last_failed_submission(&self) -> Result<CommandResult, ApplicationError> {
        let state = self.current_state()?;
        let Some(input) = state.last_submission_input else {
            return Err(ApplicationError::InvalidPayload(
                "Es gibt keinen wiederholbaren Vorgang".into(),
            ));
        };
        self.start_submission(Some(input))
    }

    fn reduce(&self, action: AppAction) -> Result<CommandResult, ApplicationError> {
        if matches!(
            action,
            AppAction::SaveNote | AppAction::ArchiveNote | AppAction::DeleteNote
        ) {
            return self.reduce_note_persistence_action(action);
        }
        let effect = match &action {
            AppAction::Focus { element_id } => Some(AppEffect::Focus {
                element_id: element_id.clone(),
            }),
            AppAction::RequestFilePicker => Some(AppEffect::OpenNativeFilePicker),
            _ => None,
        };
        let theme_mode_to_persist = match &action {
            AppAction::SetThemeMode { mode } => Some(*mode),
            _ => None,
        };
        match self
            .publisher
            .update(effect, |state| AppReducer::apply(state, &action))
        {
            Ok((message, state)) => {
                if let Some(mode) = theme_mode_to_persist {
                    if let Err(error) = self.service.save_theme_mode(mode.as_str()) {
                        tracing::warn!(%error, "Darstellungsmodus konnte nicht gespeichert werden");
                    }
                }
                Ok(CommandResult { message, state })
            }
            Err(error) => {
                self.publisher.publish_event(AppEvent::ActionFailed {
                    error: UiError::from_application_error(&error),
                })?;
                Err(error)
            }
        }
    }

    fn reduce_note_persistence_action(
        &self,
        action: AppAction,
    ) -> Result<CommandResult, ApplicationError> {
        let result = self.persist_note_action(action);
        if let Err(error) = &result {
            self.publisher.publish_event(AppEvent::ActionFailed {
                error: UiError::from_application_error(error),
            })?;
        }
        result
    }

    fn persist_note_action(&self, action: AppAction) -> Result<CommandResult, ApplicationError> {
        let current = self.current_state()?;
        let message = match action {
            AppAction::SaveNote => {
                if let Some(id) = current.selected_note_id.as_deref() {
                    let id = domain::NoteId::new(id.to_string())?;
                    self.note_service
                        .update_note(&id, current.note_title, current.note_body)?;
                    "Notiz gespeichert".to_string()
                } else {
                    self.note_service
                        .create_note(current.note_title, current.note_body)?;
                    "Notiz erstellt".to_string()
                }
            }
            AppAction::ArchiveNote => {
                let id = current.selected_note_id.ok_or_else(|| {
                    ApplicationError::InvalidPayload("Keine Notiz ausgewählt".into())
                })?;
                let id = domain::NoteId::new(id)?;
                self.note_service.archive_note(&id)?;
                "Notiz archiviert".to_string()
            }
            AppAction::DeleteNote => {
                let id = current.selected_note_id.ok_or_else(|| {
                    ApplicationError::InvalidPayload("Keine Notiz ausgewählt".into())
                })?;
                let id = domain::NoteId::new(id)?;
                self.note_service.delete_note(&id)?;
                "Notiz gelöscht".to_string()
            }
            _ => unreachable!("only note persistence actions are handled here"),
        };
        let notes: Vec<NoteListItem> = self
            .note_service
            .list_active_notes()?
            .into_iter()
            .map(Into::into)
            .collect();
        let (_, state) = self.publisher.update(None, |state| {
            state.replace_notes(notes);
            state.error_message = None;
            Ok(message.clone())
        })?;
        Ok(CommandResult { message, state })
    }

    fn start_submission(
        &self,
        retry_input: Option<String>,
    ) -> Result<CommandResult, ApplicationError> {
        let task_id = format!("task-{}", self.task_sequence.fetch_add(1, Ordering::AcqRel));
        let cancellation = Arc::new(AtomicBool::new(false));
        {
            let active_count = self
                .cancellations
                .lock()
                .map_err(|_| ApplicationError::Repository("Abbruchsperre beschädigt".into()))?
                .values()
                .filter(|token| !token.load(Ordering::Acquire))
                .count();
            if active_count >= MAX_PARALLEL_TASKS {
                let error = ApplicationError::InvalidPayload("Task-Limit erreicht".into());
                self.publisher.publish_event(AppEvent::ActionFailed {
                    error: UiError {
                        code: ErrorCode::TaskQueueFull,
                        severity: ErrorSeverity::Warning,
                        user_message: error.user_message(),
                        diagnostic_message: error.diagnostic_message(),
                        recoverable: true,
                    },
                })?;
                return Err(error);
            }
        }
        let (payload, started) =
            match self
                .publisher
                .begin_submission(task_id.clone(), retry_input, "Übermittlung")
            {
                Ok(started) => started,
                Err(error) => {
                    self.publisher.publish_event(AppEvent::ActionFailed {
                        error: UiError::from_application_error(&error),
                    })?;
                    return Err(error);
                }
            };
        self.cancellations
            .lock()
            .map_err(|_| ApplicationError::Repository("Abbruchsperre beschädigt".into()))?
            .insert(task_id.clone(), Arc::clone(&cancellation));
        Self::start_submission_worker(
            task_id,
            self.service.clone(),
            self.publisher.clone(),
            Arc::clone(&self.cancellations),
            QueuedSubmission { payload },
            cancellation,
        );
        Ok(started)
    }

    fn start_submission_worker(
        task_id: String,
        service: AppService<R>,
        publisher: StatePublisher,
        cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
        queued: QueuedSubmission,
        cancellation: Arc<AtomicBool>,
    ) {
        thread::spawn(move || {
            if cancellation.load(Ordering::Acquire) {
                let _ = publisher.publish_event(AppEvent::SubmissionCancelled {
                    task_id: task_id.clone(),
                });
                remove_cancellation(&cancellations, &task_id);
                return;
            }
            let _ = publisher.publish_event(AppEvent::SubmissionProgress {
                task_id: task_id.clone(),
                progress: 40,
            });
            let event = match service.submit_with_cancellation(queued.payload, &cancellation) {
                Ok(()) => AppEvent::SubmissionSucceeded {
                    task_id: task_id.clone(),
                },
                Err(_) if cancellation.load(Ordering::Acquire) => AppEvent::SubmissionCancelled {
                    task_id: task_id.clone(),
                },
                Err(error) => AppEvent::SubmissionFailed {
                    task_id: task_id.clone(),
                    error: UiError::from_application_error(&error),
                },
            };
            if cancellation.load(Ordering::Acquire) {
                let _ = publisher.publish_event(AppEvent::SubmissionCancelled {
                    task_id: task_id.clone(),
                });
                remove_cancellation(&cancellations, &task_id);
                return;
            }
            if let Err(error) = publisher.publish_event(event) {
                tracing::error!(%error, "Übermittlungsstatus konnte nicht veröffentlicht werden");
            }
            remove_cancellation(&cancellations, &task_id);
        });
    }
}

fn remove_cancellation(cancellations: &Mutex<HashMap<String, Arc<AtomicBool>>>, task_id: &str) {
    match cancellations.lock() {
        Ok(mut cancellations) => {
            cancellations.remove(task_id);
        }
        Err(error) => tracing::error!(%error, "Abbruchstatus konnte nicht bereinigt werden"),
    }
}

#[derive(Debug)]
struct QueuedSubmission {
    payload: SubmissionPayload,
}

impl StatePublisher {
    fn begin_submission(
        &self,
        task_id: String,
        retry_input: Option<String>,
        title: &str,
    ) -> Result<(SubmissionPayload, CommandResult), ApplicationError> {
        let ((message, input), state) = self.update(None, |state| {
            let input = retry_input.unwrap_or_else(|| state.input.clone());
            if input.trim().is_empty() {
                state
                    .set_field_validation("main.input", Some("Wert ist erforderlich.".to_string()));
                return Err(domain::DomainError::EmptyValue.into());
            }
            state.last_submission_input = Some(input.clone());
            let message = AppReducer::apply_event(
                state,
                &AppEvent::SubmissionStarted {
                    task: TaskState::running(task_id, title),
                },
            );
            Ok((message, input))
        })?;
        Ok((
            SubmissionPayload { value: input },
            CommandResult { message, state },
        ))
    }

    fn current_state(&self) -> Result<AppState, ApplicationError> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| ApplicationError::Repository("Zustandssperre beschädigt".into()))
    }

    fn subscribe(&self) -> Result<Receiver<StateUpdate>, ApplicationError> {
        let (sender, receiver) = mpsc::channel();
        self.subscribers
            .lock()
            .map_err(|_| ApplicationError::Repository("Abonnementsperre beschädigt".into()))?
            .push(sender);
        Ok(receiver)
    }

    fn publish_event(&self, event: AppEvent) -> Result<CommandResult, ApplicationError> {
        let (message, state) =
            self.update(None, |state| Ok(AppReducer::apply_event(state, &event)))?;
        Ok(CommandResult { message, state })
    }

    fn update<T, F>(
        &self,
        effect: Option<AppEffect>,
        update: F,
    ) -> Result<(T, AppState), ApplicationError>
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
            return Ok((result, state.clone()));
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
            state: updated_state.clone(),
            effect,
        };
        subscribers.retain(|subscriber| subscriber.send(update.clone()).is_ok());
        Ok((result, updated_state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone, Utc};
    use domain::{AppId, AppSettings, Note, NoteId, SubmissionRecord};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Condvar,
    };
    use std::time::Duration;

    const TEST_ID: &str = "01890f3e-8c00-7b9a-a6d1-2f0a3b4c5d6e";

    #[derive(Clone)]
    struct FixedClock {
        now: DateTime<Utc>,
        calls: Arc<AtomicUsize>,
    }

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.now
        }
    }

    #[derive(Clone, Copy)]
    struct FixedIdGenerator;

    impl IdGenerator for FixedIdGenerator {
        fn generate(&self, _created_at: DateTime<Utc>) -> Result<AppId, ApplicationError> {
            AppId::new(TEST_ID).map_err(ApplicationError::from)
        }
    }

    #[derive(Clone, Default)]
    struct FakeRepository {
        records: Arc<Mutex<Vec<SubmissionRecord>>>,
        notes: Arc<Mutex<Vec<Note>>>,
    }

    impl AppRepository for FakeRepository {
        fn load_settings(&self) -> Result<AppSettings, ApplicationError> {
            Ok(AppSettings::default())
        }
        fn save_settings(&self, _settings: AppSettings) -> Result<(), ApplicationError> {
            Ok(())
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

    impl NoteRepository for FakeRepository {
        fn create_note(&self, note: Note) -> Result<(), ApplicationError> {
            self.notes
                .lock()
                .map_err(|_| ApplicationError::Repository("Notizsperre beschädigt".into()))?
                .push(note);
            Ok(())
        }

        fn list_notes(&self) -> Result<Vec<Note>, ApplicationError> {
            self.notes
                .lock()
                .map(|notes| notes.clone())
                .map_err(|_| ApplicationError::Repository("Notizsperre beschädigt".into()))
        }

        fn update_note(&self, note: Note) -> Result<(), ApplicationError> {
            let mut notes = self
                .notes
                .lock()
                .map_err(|_| ApplicationError::Repository("Notizsperre beschädigt".into()))?;
            if let Some(existing) = notes.iter_mut().find(|existing| existing.id() == note.id()) {
                *existing = note;
            }
            Ok(())
        }

        fn delete_note(&self, id: &NoteId) -> Result<(), ApplicationError> {
            self.notes
                .lock()
                .map_err(|_| ApplicationError::Repository("Notizsperre beschädigt".into()))?
                .retain(|note| note.id() != id);
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct BlockingRepository {
        gate: Arc<(Mutex<BlockingRepositoryState>, Condvar)>,
    }

    #[derive(Default)]
    struct BlockingRepositoryState {
        entered: bool,
        released: bool,
        records: Vec<SubmissionRecord>,
    }

    impl AppRepository for BlockingRepository {
        fn load_settings(&self) -> Result<AppSettings, ApplicationError> {
            Ok(AppSettings::default())
        }
        fn save_settings(&self, _settings: AppSettings) -> Result<(), ApplicationError> {
            Ok(())
        }

        fn save_submission(&self, submission: SubmissionRecord) -> Result<(), ApplicationError> {
            let (lock, changed) = &*self.gate;
            let mut state = lock
                .lock()
                .map_err(|_| ApplicationError::Repository("Testsperre beschädigt".into()))?;
            state.entered = true;
            changed.notify_all();
            while !state.released {
                state = changed
                    .wait(state)
                    .map_err(|_| ApplicationError::Repository("Testsperre beschädigt".into()))?;
            }
            state.records.push(submission);
            Ok(())
        }

        fn list_submissions(&self) -> Result<Vec<SubmissionRecord>, ApplicationError> {
            let (lock, _) = &*self.gate;
            lock.lock()
                .map(|state| state.records.clone())
                .map_err(|_| ApplicationError::Repository("Testsperre beschädigt".into()))
        }
    }

    impl NoteRepository for BlockingRepository {
        fn create_note(&self, _note: Note) -> Result<(), ApplicationError> {
            Ok(())
        }

        fn list_notes(&self) -> Result<Vec<Note>, ApplicationError> {
            Ok(Vec::new())
        }

        fn update_note(&self, _note: Note) -> Result<(), ApplicationError> {
            Ok(())
        }

        fn delete_note(&self, _id: &NoteId) -> Result<(), ApplicationError> {
            Ok(())
        }
    }

    fn fixed_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 17, 19, 10, 29)
            .single()
            .unwrap()
    }

    fn test_service(repository: FakeRepository) -> AppService<FakeRepository> {
        AppService::with_dependencies(
            repository,
            FixedClock {
                now: fixed_time(),
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedIdGenerator,
        )
    }

    #[test]
    fn submitting_persists_a_record_with_one_captured_time_and_injected_id() {
        let clock_calls = Arc::new(AtomicUsize::new(0));
        let service = AppService::with_dependencies(
            FakeRepository::default(),
            FixedClock {
                now: fixed_time(),
                calls: Arc::clone(&clock_calls),
            },
            FixedIdGenerator,
        );
        service
            .submit(SubmissionPayload {
                value: "Test".into(),
            })
            .unwrap();

        let records = service.list_records().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].description, "Test");
        assert_eq!(records[0].id.as_str(), TEST_ID);
        assert_eq!(records[0].created_at, fixed_time());
        assert_eq!(clock_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn state_store_is_shared_by_commands_and_subscribers() {
        let store = AppStateStore::new(test_service(FakeRepository::default()));
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
        let store = AppStateStore::new(test_service(FakeRepository::default()));

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
    fn state_store_publishes_inline_validation_failure() {
        let store = AppStateStore::new(test_service(FakeRepository::default()));
        let updates = store.subscribe().unwrap();

        store.submit().unwrap_err();
        let failed = updates.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(failed.state.status, domain::AppStatus::Error);
        assert_eq!(
            failed.state.error_message.as_deref(),
            Some("Bitte füllen Sie das Pflichtfeld aus.")
        );
        assert_eq!(
            failed.state.validation_message("main.input"),
            Some("Wert ist erforderlich.")
        );

        let state = store.current_state().unwrap();
        assert!(!state.busy);
        assert_eq!(state.status, domain::AppStatus::Error);
        assert_eq!(
            state.error_message.as_deref(),
            Some("Bitte füllen Sie das Pflichtfeld aus.")
        );
    }

    #[test]
    fn state_store_publishes_submission_lifecycle() {
        let store = AppStateStore::new(test_service(FakeRepository::default()));
        let updates = store.subscribe().unwrap();
        store
            .dispatch(AppAction::SetInput {
                value: "Test".into(),
            })
            .unwrap();
        updates.recv().unwrap();

        let result = store.submit().unwrap();
        assert_eq!(result.state.status, domain::AppStatus::Busy);
        assert!(result.state.busy);
        assert_eq!(
            updates
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .state
                .status,
            domain::AppStatus::Busy
        );
        let final_update = wait_for_status(&updates, domain::AppStatus::Success);
        assert_eq!(final_update.status, domain::AppStatus::Success);
        let state = store.current_state().unwrap();
        assert_eq!(state.status, domain::AppStatus::Success);
        assert!(!state.busy);
    }

    #[test]
    fn submissions_have_task_ids_and_can_run_in_parallel() {
        let repository = BlockingRepository::default();
        let store = AppStateStore::new(AppService::with_dependencies(
            repository.clone(),
            FixedClock {
                now: fixed_time(),
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedIdGenerator,
        ));
        store
            .dispatch(AppAction::SetInput {
                value: "Erster Wert".into(),
            })
            .unwrap();

        let first = store.submit().unwrap();
        let second = store.submit().unwrap();

        assert_ne!(first.state.active_task_id, second.state.active_task_id);
        let state = store.current_state().unwrap();
        assert_eq!(state.tasks.len(), 2);
        assert!(state.tasks.iter().all(TaskState::is_active));
        let (lock, changed) = &*repository.gate;
        let mut repository_state = lock.lock().unwrap();
        repository_state.released = true;
        changed.notify_all();
    }

    #[test]
    fn failed_submission_publishes_structured_retryable_error() {
        let store = AppStateStore::new(test_service(FakeRepository::default()));
        let updates = store.subscribe().unwrap();

        store.submit().unwrap_err();
        let state = wait_for_status(&updates, domain::AppStatus::Error);

        let error = state.ui_error.as_ref().expect("structured error");
        assert_eq!(error.code, ErrorCode::ValidationRequired);
        assert_eq!(error.user_message, "Bitte füllen Sie das Pflichtfeld aus.");
        assert!(error.recoverable);
        assert!(!state.can_retry());
    }

    #[test]
    fn retry_uses_last_failed_submission_payload() {
        let store = AppStateStore::new(test_service(FakeRepository::default()));
        store
            .dispatch(AppAction::SetInput {
                value: "Wiederholen".into(),
            })
            .unwrap();
        store.submit().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            store.current_state().unwrap().status,
            domain::AppStatus::Success
        );

        store
            .publisher
            .publish_event(AppEvent::SubmissionFailed {
                task_id: "task-1".into(),
                error: UiError::task_failed("simulierter Fehler".into()),
            })
            .unwrap();

        store.dispatch(AppAction::RetryLastFailedTask).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(store.service().list_records().unwrap().len(), 2);
    }

    #[test]
    fn reset_clears_progress_from_a_previous_submission() {
        let repository = FakeRepository::default();
        let store = AppStateStore::new(AppService::new(repository));
        store
            .dispatch(AppAction::SetInput {
                value: "Wert".into(),
            })
            .unwrap();
        store.submit().unwrap();
        store.dispatch(AppAction::Reset).unwrap();
        let state = store.current_state().unwrap();
        assert_eq!(state.progress, None);
        assert!(!state.busy);
    }

    #[test]
    fn submission_repository_work_runs_on_the_bounded_worker() {
        let repository = BlockingRepository::default();
        let service = AppService::with_dependencies(
            repository.clone(),
            FixedClock {
                now: fixed_time(),
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedIdGenerator,
        );
        let store = AppStateStore::new(service);
        let updates = store.subscribe().unwrap();
        store
            .dispatch(AppAction::SetInput {
                value: "Test".into(),
            })
            .unwrap();
        updates.recv_timeout(Duration::from_secs(1)).unwrap();

        let caller = store.clone();
        let (result_sender, result_receiver) = mpsc::channel();
        let submit_thread = thread::spawn(move || {
            result_sender.send(caller.submit()).unwrap();
        });
        let result = result_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("submit must return before repository work completes")
            .unwrap();
        assert_eq!(result.state.status, domain::AppStatus::Busy);
        assert_eq!(
            updates
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .state
                .status,
            domain::AppStatus::Busy
        );

        let (lock, changed) = &*repository.gate;
        let state = lock.lock().unwrap();
        let (mut state, wait_result) = changed
            .wait_timeout_while(state, Duration::from_secs(1), |state| !state.entered)
            .unwrap();
        assert!(!wait_result.timed_out());
        assert!(state.records.is_empty());
        state.released = true;
        changed.notify_all();
        drop(state);

        submit_thread.join().unwrap();
        let final_update = wait_for_status(&updates, domain::AppStatus::Success);
        assert_eq!(final_update.status, domain::AppStatus::Success);
        assert_eq!(repository.list_submissions().unwrap().len(), 1);
    }

    #[test]
    fn state_store_does_not_publish_unchanged_state() {
        let store = AppStateStore::new(test_service(FakeRepository::default()));
        let updates = store.subscribe().unwrap();

        store.dispatch(AppAction::OpenAbout).unwrap();
        updates.recv().unwrap();
        let revision = store.current_state().unwrap().revision;

        store.dispatch(AppAction::OpenAbout).unwrap();

        assert!(updates.try_recv().is_err());
        assert_eq!(store.current_state().unwrap().revision, revision);
    }

    fn wait_for_status(updates: &Receiver<StateUpdate>, expected: domain::AppStatus) -> AppState {
        for _ in 0..8 {
            let update = updates.recv_timeout(Duration::from_secs(1)).unwrap();
            if update.state.status == expected {
                return update.state;
            }
        }
        panic!("status {expected:?} was not published");
    }

    #[test]
    fn settings_and_theme_mode_share_the_application_state() {
        let store = AppStateStore::new(test_service(FakeRepository::default()));

        assert_eq!(store.current_state().unwrap().theme_mode, ThemeMode::System);
        store.dispatch(AppAction::OpenSettings).unwrap();
        assert_eq!(
            store.current_state().unwrap().screen(),
            domain::AppScreen::Settings
        );

        store
            .dispatch(AppAction::SetThemeMode {
                mode: ThemeMode::Dark,
            })
            .unwrap();
        let state = store.current_state().unwrap();
        assert_eq!(state.theme_mode, ThemeMode::Dark);
        assert!(state.is_settings_open());

        store.dispatch(AppAction::CloseSettings).unwrap();
        let state = store.current_state().unwrap();
        assert_eq!(state.screen(), domain::AppScreen::Main);
        assert_eq!(state.theme_mode, ThemeMode::Dark);
    }

    #[test]
    fn theme_mode_parser_rejects_unknown_values() {
        assert_eq!(ThemeMode::parse("system").unwrap(), ThemeMode::System);
        assert_eq!(ThemeMode::parse("light").unwrap(), ThemeMode::Light);
        assert_eq!(ThemeMode::parse("dark").unwrap(), ThemeMode::Dark);
        assert!(ThemeMode::parse("automatic")
            .unwrap_err()
            .to_string()
            .contains("unbekannter Darstellungsmodus"));
    }

    #[test]
    fn note_workflow_is_persisted_and_reflected_in_state() {
        let repository = FakeRepository::default();
        let store = AppStateStore::new(test_service(repository.clone()));

        store
            .dispatch(AppAction::SetNoteTitle {
                value: "Erste Notiz".into(),
            })
            .unwrap();
        store
            .dispatch(AppAction::SetNoteBody {
                value: "Inhalt".into(),
            })
            .unwrap();
        store.dispatch(AppAction::SaveNote).unwrap();

        let state = store.current_state().unwrap();
        assert_eq!(state.notes.len(), 1);
        assert_eq!(state.note_title, "Erste Notiz");
        assert!(state.selected_note_id.is_some());
        assert_eq!(repository.list_notes().unwrap().len(), 1);

        store
            .dispatch(AppAction::SetNoteTitle {
                value: "Umbenannt".into(),
            })
            .unwrap();
        store.dispatch(AppAction::SaveNote).unwrap();
        assert_eq!(store.current_state().unwrap().notes[0].title, "Umbenannt");

        store.dispatch(AppAction::ArchiveNote).unwrap();
        let state = store.current_state().unwrap();
        assert!(state.notes.is_empty());
        assert!(state.selected_note_id.is_none());
        assert!(repository.list_notes().unwrap()[0].is_archived());
    }

    #[test]
    fn focusing_a_control_publishes_a_transient_effect_without_mutating_state() {
        let store = AppStateStore::new(test_service(FakeRepository::default()));
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
        let store = AppStateStore::new(test_service(FakeRepository::default()));
        let error = store
            .dispatch(AppAction::SelectFile { path: "\0".into() })
            .unwrap_err();
        assert!(error.to_string().contains("enthält ungültige Nullbytes"));
        assert_eq!(store.current_state().unwrap().selected_file, "");
    }

    #[test]
    fn file_picker_request_is_published_as_a_transient_effect() {
        let store = AppStateStore::new(test_service(FakeRepository::default()));
        let updates = store.subscribe().unwrap();

        store.dispatch(AppAction::RequestFilePicker).unwrap();
        assert_eq!(
            updates.recv_timeout(Duration::from_secs(1)).unwrap().effect,
            Some(AppEffect::OpenNativeFilePicker)
        );

        let revision = store.current_state().unwrap().revision;
        store.dispatch(AppAction::RequestFilePicker).unwrap();
        assert_eq!(
            updates.recv_timeout(Duration::from_secs(1)).unwrap().effect,
            Some(AppEffect::OpenNativeFilePicker)
        );
        assert_eq!(store.current_state().unwrap().revision, revision);

        store.dispatch(AppAction::CancelFileSelection).unwrap();
        assert!(updates.try_recv().is_err());
    }

    #[test]
    fn rejected_actions_are_reported_through_canonical_state() {
        let store = AppStateStore::new(test_service(FakeRepository::default()));

        let error = store
            .dispatch(AppAction::SelectFile { path: "\0".into() })
            .unwrap_err();

        let state = store.current_state().unwrap();
        assert_eq!(state.status, domain::AppStatus::Error);
        assert_eq!(
            state.error_message.as_deref(),
            Some(error.user_message().as_str())
        );
        assert_eq!(
            state.ui_error.as_ref().map(|error| error.code),
            Some(ErrorCode::InvalidPayload)
        );
    }
}
