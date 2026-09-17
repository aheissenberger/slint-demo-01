use domain::{AppScreen, AppStatus, DomainError, Note};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("Domänenfehler: {0}")]
    Domain(#[from] DomainError),
    #[error("Repository-Fehler: {0}")]
    Repository(String),
    #[error("ungültige Nutzdaten: {0}")]
    InvalidPayload(String),
    #[error("Datenwiederherstellung erforderlich: {0}")]
    DataRecoveryRequired(String),
}

impl ApplicationError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Domain(DomainError::EmptyValue) => ErrorCode::ValidationRequired,
            Self::Domain(DomainError::ValueTooLong) => ErrorCode::ValidationTooLong,
            Self::Domain(DomainError::InvalidNoteTitle) => ErrorCode::ValidationRequired,
            Self::Domain(DomainError::NoteTitleTooLong | DomainError::NoteBodyTooLong) => {
                ErrorCode::ValidationTooLong
            }
            Self::Domain(_) => ErrorCode::DomainInvalidState,
            Self::Repository(_) => ErrorCode::PersistenceUnavailable,
            Self::InvalidPayload(_) => ErrorCode::InvalidPayload,
            Self::DataRecoveryRequired(_) => ErrorCode::PersistenceRecoveryRequired,
        }
    }

    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::DataRecoveryRequired(_) => ErrorSeverity::Critical,
            Self::Repository(_) => ErrorSeverity::Error,
            Self::Domain(_) | Self::InvalidPayload(_) => ErrorSeverity::Warning,
        }
    }

    pub fn user_message(&self) -> String {
        match self.code() {
            ErrorCode::ValidationRequired => "Bitte füllen Sie das Pflichtfeld aus.".into(),
            ErrorCode::ValidationTooLong => "Die Eingabe ist zu lang.".into(),
            ErrorCode::DomainInvalidState => {
                "Der Vorgang ist im aktuellen Zustand nicht möglich.".into()
            }
            ErrorCode::InvalidPayload => "Die Eingabe konnte nicht verarbeitet werden.".into(),
            ErrorCode::PersistenceUnavailable => {
                "Die lokalen Anwendungsdaten konnten nicht gespeichert oder gelesen werden.".into()
            }
            ErrorCode::PersistenceRecoveryRequired => {
                "Die lokalen Anwendungsdaten sind beschädigt oder inkompatibel.".into()
            }
            ErrorCode::TaskCancelled => "Der Vorgang wurde abgebrochen.".into(),
            ErrorCode::TaskQueueFull => "Es laufen bereits zu viele Vorgänge.".into(),
        }
    }

    pub fn diagnostic_message(&self) -> String {
        self.to_string()
    }

    pub fn recoverable(&self) -> bool {
        !matches!(self, Self::DataRecoveryRequired(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ValidationRequired,
    ValidationTooLong,
    DomainInvalidState,
    InvalidPayload,
    PersistenceUnavailable,
    PersistenceRecoveryRequired,
    TaskCancelled,
    TaskQueueFull,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::ValidationRequired => "validation_required",
            Self::ValidationTooLong => "validation_too_long",
            Self::DomainInvalidState => "domain_invalid_state",
            Self::InvalidPayload => "invalid_payload",
            Self::PersistenceUnavailable => "persistence_unavailable",
            Self::PersistenceRecoveryRequired => "persistence_recovery_required",
            Self::TaskCancelled => "task_cancelled",
            Self::TaskQueueFull => "task_queue_full",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiError {
    pub code: ErrorCode,
    pub severity: ErrorSeverity,
    pub user_message: String,
    pub diagnostic_message: String,
    pub recoverable: bool,
}

impl UiError {
    pub fn from_application_error(error: &ApplicationError) -> Self {
        Self {
            code: error.code(),
            severity: error.severity(),
            user_message: error.user_message(),
            diagnostic_message: error.diagnostic_message(),
            recoverable: error.recoverable(),
        }
    }

    pub fn task_failed(message: String) -> Self {
        Self {
            code: ErrorCode::PersistenceUnavailable,
            severity: ErrorSeverity::Error,
            user_message: "Der Vorgang konnte nicht abgeschlossen werden.".into(),
            diagnostic_message: message,
            recoverable: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldValidation {
    pub field_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Queued => "wartet",
            Self::Running => "läuft",
            Self::Succeeded => "erfolgreich",
            Self::Failed => "fehlgeschlagen",
            Self::Cancelled => "abgebrochen",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskState {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub progress: Option<u8>,
    pub error: Option<UiError>,
    pub retryable: bool,
}

impl TaskState {
    pub fn running(id: String, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            status: TaskStatus::Running,
            progress: Some(0),
            error: None,
            retryable: false,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status, TaskStatus::Queued | TaskStatus::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState {
    pub revision: u64,
    pub input: String,
    pub selected_file: String,
    pub status: AppStatus,
    pub error_message: Option<String>,
    pub busy: bool,
    pub progress: Option<u8>,
    pub active_dialog: Option<ActiveDialog>,
    pub theme_mode: ThemeMode,
    pub notes: Vec<NoteListItem>,
    pub selected_note_id: Option<String>,
    pub note_title: String,
    pub note_body: String,
    pub validation_errors: Vec<FieldValidation>,
    pub ui_error: Option<UiError>,
    pub tasks: Vec<TaskState>,
    pub active_task_id: Option<String>,
    pub last_failed_task_id: Option<String>,
    pub last_submission_input: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            revision: 0,
            input: String::new(),
            selected_file: String::new(),
            status: AppStatus::Ready,
            error_message: None,
            busy: false,
            progress: None,
            active_dialog: None,
            theme_mode: ThemeMode::System,
            notes: Vec::new(),
            selected_note_id: None,
            note_title: String::new(),
            note_body: String::new(),
            validation_errors: Vec::new(),
            ui_error: None,
            tasks: Vec::new(),
            active_task_id: None,
            last_failed_task_id: None,
            last_submission_input: None,
        }
    }

    pub fn screen(&self) -> AppScreen {
        match self.active_dialog {
            Some(ActiveDialog::About) => AppScreen::About,
            Some(ActiveDialog::Settings) => AppScreen::Settings,
            Some(ActiveDialog::CriticalError) => AppScreen::Main,
            None => AppScreen::Main,
        }
    }

    pub fn is_about_open(&self) -> bool {
        self.active_dialog == Some(ActiveDialog::About)
    }

    pub fn is_settings_open(&self) -> bool {
        self.active_dialog == Some(ActiveDialog::Settings)
    }

    pub fn can_submit(&self) -> bool {
        !self.input.trim().is_empty()
    }

    pub fn can_save_note(&self) -> bool {
        !self.note_title.trim().is_empty()
            && !self
                .validation_errors
                .iter()
                .any(|error| error.field_id == "notes.title" || error.field_id == "notes.body")
    }

    pub fn can_retry(&self) -> bool {
        self.last_failed_task_id
            .as_deref()
            .and_then(|id| self.tasks.iter().find(|task| task.id == id))
            .is_some_and(|task| task.retryable)
            && !self.busy
            && self.last_submission_input.is_some()
    }

    pub fn visible_error(&self) -> Option<&UiError> {
        self.ui_error.as_ref()
    }

    pub fn critical_error(&self) -> Option<&UiError> {
        self.ui_error
            .as_ref()
            .filter(|error| error.severity == ErrorSeverity::Critical)
    }

    pub fn validation_message(&self, field_id: &str) -> Option<&str> {
        self.validation_errors
            .iter()
            .find(|error| error.field_id == field_id)
            .map(|error| error.message.as_str())
    }

    pub fn selected_note(&self) -> Option<&NoteListItem> {
        self.selected_note_id
            .as_deref()
            .and_then(|id| self.notes.iter().find(|note| note.id == id))
    }

    pub fn replace_notes(&mut self, notes: Vec<NoteListItem>) {
        self.notes = notes;
        if let Some(selected_id) = self.selected_note_id.as_deref() {
            if let Some(selected) = self.notes.iter().find(|note| note.id == selected_id) {
                self.note_title = selected.title.clone();
                self.note_body = selected.body.clone();
                return;
            }
        }
        if let Some(first) = self.notes.first() {
            self.selected_note_id = Some(first.id.clone());
            self.note_title = first.title.clone();
            self.note_body = first.body.clone();
        } else {
            self.selected_note_id = None;
            self.note_title.clear();
            self.note_body.clear();
        }
    }

    fn apply_action(&mut self, action: &AppAction) -> Result<String, ApplicationError> {
        match action {
            AppAction::SetInput { value } => {
                if value.len() > 4096 {
                    return Err(ApplicationError::InvalidPayload(
                        "Eingabe überschreitet 4096 Zeichen".into(),
                    ));
                }
                self.input = value.clone();
                self.error_message = None;
                self.ui_error = None;
                self.set_field_validation("main.input", None);
                Ok("Wert aktualisiert".to_string())
            }
            AppAction::Focus { element_id } => Ok(format!("{element_id} fokussiert")),
            AppAction::SelectFile { path } => {
                let path = AppReducer::validate_file_path(path)?;
                self.selected_file = path;
                Ok("Datei ausgewählt".to_string())
            }
            AppAction::RequestFilePicker => Ok("Dateiauswahl geöffnet".to_string()),
            AppAction::CancelFileSelection => Ok("Dateiauswahl abgebrochen".to_string()),
            AppAction::CancelSubmission => Ok("Vorgang wird abgebrochen".to_string()),
            AppAction::Reset => Ok(self.reset_state()),
            AppAction::OpenAbout
            | AppAction::CloseAbout
            | AppAction::OpenSettings
            | AppAction::CloseSettings
            | AppAction::SetThemeMode { .. } => self.apply_shell_action(action),
            AppAction::StartNewNote
            | AppAction::SelectNote { .. }
            | AppAction::SetNoteTitle { .. }
            | AppAction::SetNoteBody { .. } => self.apply_note_edit_action(action),
            AppAction::DismissError => Ok(self.dismiss_error()),
            AppAction::RetryLastFailedTask => Ok("Wiederholung wird vorbereitet".to_string()),
            AppAction::SaveNote | AppAction::ArchiveNote | AppAction::DeleteNote => {
                Ok("Notizen aktualisiert".to_string())
            }
        }
    }

    fn reset_state(&mut self) -> String {
        self.input.clear();
        self.status = AppStatus::Ready;
        self.progress = None;
        self.busy = false;
        self.error_message = None;
        self.ui_error = None;
        self.validation_errors.clear();
        self.tasks.clear();
        self.active_task_id = None;
        self.last_failed_task_id = None;
        self.last_submission_input = None;
        self.active_dialog = None;
        "zurückgesetzt".to_string()
    }

    fn apply_shell_action(&mut self, action: &AppAction) -> Result<String, ApplicationError> {
        match action {
            AppAction::OpenAbout => {
                self.active_dialog = Some(ActiveDialog::About);
                Ok("Info-Dialog geöffnet".to_string())
            }
            AppAction::CloseAbout => {
                if self.is_about_open() {
                    self.active_dialog = None;
                }
                Ok("Info-Dialog geschlossen".to_string())
            }
            AppAction::OpenSettings => {
                self.active_dialog = Some(ActiveDialog::Settings);
                Ok("Einstellungen geöffnet".to_string())
            }
            AppAction::CloseSettings => {
                if self.is_settings_open() {
                    self.active_dialog = None;
                }
                Ok("Einstellungen geschlossen".to_string())
            }
            AppAction::SetThemeMode { mode } => {
                self.theme_mode = *mode;
                Ok(format!("App-Design auf {} gesetzt", mode.label()))
            }
            _ => unreachable!("shell action dispatch is constrained by apply_action"),
        }
    }

    fn apply_note_edit_action(&mut self, action: &AppAction) -> Result<String, ApplicationError> {
        match action {
            AppAction::StartNewNote => {
                self.selected_note_id = None;
                self.note_title.clear();
                self.note_body.clear();
                self.error_message = None;
                self.ui_error = None;
                self.clear_note_validation();
                Ok("Neue Notiz vorbereitet".to_string())
            }
            AppAction::SelectNote { id } => self.select_note_for_editing(id),
            AppAction::SetNoteTitle { value } => {
                self.note_title = value.clone();
                self.error_message = None;
                self.ui_error = None;
                self.validate_note_title();
                Ok("Notiztitel aktualisiert".to_string())
            }
            AppAction::SetNoteBody { value } => {
                self.note_body = value.clone();
                self.error_message = None;
                self.ui_error = None;
                self.validate_note_body();
                Ok("Notizinhalt aktualisiert".to_string())
            }
            _ => unreachable!("note edit action dispatch is constrained by apply_action"),
        }
    }

    fn select_note_for_editing(&mut self, id: &str) -> Result<String, ApplicationError> {
        let note = self
            .notes
            .iter()
            .find(|note| note.id == id)
            .ok_or_else(|| {
                ApplicationError::InvalidPayload(format!("Notiz wurde nicht gefunden: {id}"))
            })?;
        self.selected_note_id = Some(note.id.clone());
        self.note_title = note.title.clone();
        self.note_body = note.body.clone();
        self.error_message = None;
        self.ui_error = None;
        self.clear_note_validation();
        Ok("Notiz ausgewählt".to_string())
    }

    fn dismiss_error(&mut self) -> String {
        if self.ui_error.is_some() {
            if self.active_dialog == Some(ActiveDialog::CriticalError) {
                self.active_dialog = None;
            }
            self.ui_error = None;
            self.error_message = None;
        }
        "Fehlerhinweis geschlossen".to_string()
    }

    fn apply_event(&mut self, event: &AppEvent) -> String {
        match event {
            AppEvent::SubmissionStarted { task } => {
                self.busy = true;
                self.progress = Some(0);
                self.status = AppStatus::Busy;
                self.error_message = None;
                self.ui_error = None;
                self.active_task_id = Some(task.id.clone());
                self.upsert_task(task.clone());
                "wird ausgeführt".to_string()
            }
            AppEvent::SubmissionProgress { task_id, progress } => {
                self.update_task(task_id, |task| {
                    task.progress = Some(*progress);
                    task.status = TaskStatus::Running;
                });
                self.progress = Some(*progress);
                "Fortschritt aktualisiert".to_string()
            }
            AppEvent::SubmissionSucceeded { task_id } => {
                self.update_task(task_id, |task| {
                    task.status = TaskStatus::Succeeded;
                    task.progress = Some(100);
                    task.error = None;
                    task.retryable = false;
                });
                self.refresh_busy_from_tasks();
                self.status = if self.busy {
                    AppStatus::Busy
                } else {
                    AppStatus::Success
                };
                self.error_message = None;
                self.ui_error = None;
                "Befehl ausgeführt".to_string()
            }
            AppEvent::SubmissionFailed { task_id, error } => {
                self.update_task(task_id, |task| {
                    task.status = TaskStatus::Failed;
                    task.progress = None;
                    task.error = Some(error.clone());
                    task.retryable = error.recoverable;
                });
                self.refresh_busy_from_tasks();
                self.status = AppStatus::Error;
                self.error_message = Some(error.user_message.clone());
                self.ui_error = Some(error.clone());
                if error.severity == ErrorSeverity::Critical {
                    self.active_dialog = Some(ActiveDialog::CriticalError);
                }
                self.last_failed_task_id = Some(task_id.clone());
                error.user_message.clone()
            }
            AppEvent::SubmissionCancelled { task_id } => {
                self.update_task(task_id, |task| {
                    task.status = TaskStatus::Cancelled;
                    task.progress = None;
                    task.error = Some(UiError {
                        code: ErrorCode::TaskCancelled,
                        severity: ErrorSeverity::Info,
                        user_message: "Der Vorgang wurde abgebrochen.".into(),
                        diagnostic_message: "submission task cancelled".into(),
                        recoverable: true,
                    });
                    task.retryable = true;
                });
                self.refresh_busy_from_tasks();
                self.status = if self.busy {
                    AppStatus::Busy
                } else {
                    AppStatus::Ready
                };
                self.error_message = None;
                self.ui_error = None;
                "Vorgang abgebrochen".to_string()
            }
            AppEvent::ActionFailed { error } => {
                self.status = AppStatus::Error;
                self.error_message = Some(error.user_message.clone());
                self.ui_error = Some(error.clone());
                if error.severity == ErrorSeverity::Critical {
                    self.active_dialog = Some(ActiveDialog::CriticalError);
                }
                error.user_message.clone()
            }
        }
    }

    pub(crate) fn set_field_validation(&mut self, field_id: &str, message: Option<String>) {
        self.validation_errors
            .retain(|error| error.field_id != field_id);
        if let Some(message) = message {
            self.validation_errors.push(FieldValidation {
                field_id: field_id.to_string(),
                message,
            });
        }
    }

    fn validate_note_title(&mut self) {
        let message = if self.note_title.trim().is_empty() {
            Some("Titel ist erforderlich.".to_string())
        } else if self.note_title.len() > 200 {
            Some("Titel darf maximal 200 Zeichen enthalten.".to_string())
        } else {
            None
        };
        self.set_field_validation("notes.title", message);
    }

    fn validate_note_body(&mut self) {
        let message = if self.note_body.len() > 8192 {
            Some("Inhalt darf maximal 8192 Zeichen enthalten.".to_string())
        } else {
            None
        };
        self.set_field_validation("notes.body", message);
    }

    fn clear_note_validation(&mut self) {
        self.validation_errors
            .retain(|error| !matches!(error.field_id.as_str(), "notes.title" | "notes.body"));
    }

    fn upsert_task(&mut self, task: TaskState) {
        if let Some(existing) = self
            .tasks
            .iter_mut()
            .find(|existing| existing.id == task.id)
        {
            *existing = task;
        } else {
            self.tasks.push(task);
        }
    }

    fn update_task(&mut self, task_id: &str, update: impl FnOnce(&mut TaskState)) {
        if let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) {
            update(task);
        }
    }

    fn refresh_busy_from_tasks(&mut self) {
        self.busy = self.tasks.iter().any(TaskState::is_active);
        self.active_task_id = self
            .tasks
            .iter()
            .rev()
            .find(|task| task.is_active())
            .map(|task| task.id.clone());
        self.progress = self
            .active_task_id
            .as_deref()
            .and_then(|id| self.tasks.iter().find(|task| task.id == id))
            .and_then(|task| task.progress);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteListItem {
    pub id: String,
    pub title: String,
    pub body: String,
    pub archived: bool,
}

impl From<Note> for NoteListItem {
    fn from(note: Note) -> Self {
        Self {
            id: note.id().as_str().to_string(),
            title: note.title().as_str().to_string(),
            body: note.body().as_str().to_string(),
            archived: note.is_archived(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveDialog {
    About,
    Settings,
    CriticalError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Systemeinstellung",
            Self::Light => "Hell",
            Self::Dark => "Dunkel",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ApplicationError> {
        match value {
            "system" => Ok(Self::System),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(ApplicationError::InvalidPayload(format!(
                "unbekannter Darstellungsmodus: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    SetInput { value: String },
    Focus { element_id: String },
    SelectFile { path: String },
    RequestFilePicker,
    CancelFileSelection,
    CancelSubmission,
    Reset,
    OpenAbout,
    CloseAbout,
    OpenSettings,
    CloseSettings,
    SetThemeMode { mode: ThemeMode },
    StartNewNote,
    SelectNote { id: String },
    SetNoteTitle { value: String },
    SetNoteBody { value: String },
    SaveNote,
    ArchiveNote,
    DeleteNote,
    RetryLastFailedTask,
    DismissError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEffect {
    Focus { element_id: String },
    OpenNativeFilePicker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    SubmissionStarted { task: TaskState },
    SubmissionProgress { task_id: String, progress: u8 },
    SubmissionSucceeded { task_id: String },
    SubmissionFailed { task_id: String, error: UiError },
    SubmissionCancelled { task_id: String },
    ActionFailed { error: UiError },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub message: String,
    pub state: AppState,
}

pub struct AppReducer;

impl AppReducer {
    pub fn apply(state: &mut AppState, action: &AppAction) -> Result<String, ApplicationError> {
        state.apply_action(action)
    }

    pub fn apply_event(state: &mut AppState, event: &AppEvent) -> String {
        state.apply_event(event)
    }

    pub fn validate_file_path(path: &str) -> Result<String, ApplicationError> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err(ApplicationError::InvalidPayload(
                "Dateipfad darf nicht leer sein".into(),
            ));
        }
        if trimmed.len() > 4096 {
            return Err(ApplicationError::InvalidPayload(
                "Dateipfad überschreitet 4096 Zeichen".into(),
            ));
        }
        if trimmed.contains('\0') {
            return Err(ApplicationError::InvalidPayload(
                "Dateipfad enthält ungültige Nullbytes".into(),
            ));
        }

        Ok(trimmed.to_string())
    }
}
