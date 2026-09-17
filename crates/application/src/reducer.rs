use domain::{AppScreen, AppStatus, DomainError, Note};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("Domänenfehler: {0}")]
    Domain(#[from] DomainError),
    #[error("Repository-Fehler: {0}")]
    Repository(String),
    #[error("ungültige Nutzdaten: {0}")]
    InvalidPayload(String),
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
        }
    }

    pub fn screen(&self) -> AppScreen {
        match self.active_dialog {
            Some(ActiveDialog::About) => AppScreen::About,
            Some(ActiveDialog::Settings) => AppScreen::Settings,
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
        !self.busy && !self.input.trim().is_empty()
    }

    pub fn can_save_note(&self) -> bool {
        !self.note_title.trim().is_empty()
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
            AppAction::Reset => {
                self.input.clear();
                self.status = AppStatus::Ready;
                self.progress = None;
                self.busy = false;
                self.error_message = None;
                self.active_dialog = None;
                Ok("zurückgesetzt".to_string())
            }
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
            AppAction::StartNewNote => {
                self.selected_note_id = None;
                self.note_title.clear();
                self.note_body.clear();
                self.error_message = None;
                Ok("Neue Notiz vorbereitet".to_string())
            }
            AppAction::SelectNote { id } => {
                let note = self
                    .notes
                    .iter()
                    .find(|note| note.id == *id)
                    .ok_or_else(|| {
                        ApplicationError::InvalidPayload(format!(
                            "Notiz wurde nicht gefunden: {id}"
                        ))
                    })?;
                self.selected_note_id = Some(note.id.clone());
                self.note_title = note.title.clone();
                self.note_body = note.body.clone();
                self.error_message = None;
                Ok("Notiz ausgewählt".to_string())
            }
            AppAction::SetNoteTitle { value } => {
                self.note_title = value.clone();
                self.error_message = None;
                Ok("Notiztitel aktualisiert".to_string())
            }
            AppAction::SetNoteBody { value } => {
                self.note_body = value.clone();
                self.error_message = None;
                Ok("Notizinhalt aktualisiert".to_string())
            }
            AppAction::SaveNote | AppAction::ArchiveNote | AppAction::DeleteNote => {
                Ok("Notizen aktualisiert".to_string())
            }
        }
    }

    fn apply_event(&mut self, event: &AppEvent) -> String {
        match event {
            AppEvent::SubmissionStarted => {
                self.busy = true;
                self.progress = Some(0);
                self.status = AppStatus::Busy;
                self.error_message = None;
                "wird ausgeführt".to_string()
            }
            AppEvent::SubmissionSucceeded => {
                self.busy = false;
                self.progress = Some(100);
                self.status = AppStatus::Success;
                self.error_message = None;
                "Befehl ausgeführt".to_string()
            }
            AppEvent::SubmissionFailed { message } => {
                self.busy = false;
                self.progress = None;
                self.status = AppStatus::Error;
                self.error_message = Some(message.clone());
                message.clone()
            }
            AppEvent::SubmissionCancelled => {
                self.busy = false;
                self.progress = None;
                self.status = AppStatus::Ready;
                self.error_message = None;
                "Vorgang abgebrochen".to_string()
            }
            AppEvent::ActionFailed { message } => {
                self.status = AppStatus::Error;
                self.error_message = Some(message.clone());
                message.clone()
            }
        }
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEffect {
    Focus { element_id: String },
    OpenNativeFilePicker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    SubmissionStarted,
    SubmissionSucceeded,
    SubmissionFailed { message: String },
    SubmissionCancelled,
    ActionFailed { message: String },
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
