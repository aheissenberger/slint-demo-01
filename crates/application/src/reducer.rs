use domain::{AppScreen, AppStatus, DomainError};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("Domänenfehler: {0}")]
    Domain(#[from] DomainError),
    #[error("Repository-Fehler: {0}")]
    Repository(String),
    #[error("ungültige Nutzdaten: {0}")]
    InvalidPayload(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AppState {
    pub revision: u64,
    pub screen: AppScreen,
    pub input: String,
    pub selected_file: String,
    pub status: AppStatus,
    pub error_message: Option<String>,
    pub busy: bool,
    pub about_open: bool,
    pub file_picker_requested: bool,
    pub focused_element: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            revision: 0,
            screen: AppScreen::Main,
            input: String::new(),
            selected_file: String::new(),
            status: AppStatus::Ready,
            error_message: None,
            busy: false,
            about_open: false,
            file_picker_requested: false,
            focused_element: None,
        }
    }

    pub fn can_submit(&self) -> bool {
        !self.busy && !self.input.trim().is_empty()
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
            AppAction::Focus { element_id } => {
                self.focused_element = Some(element_id.clone());
                Ok(format!("{element_id} fokussiert"))
            }
            AppAction::ClearFocus => {
                self.focused_element = None;
                Ok("Fokus aktualisiert".to_string())
            }
            AppAction::SelectFile { path } => {
                let path = AppReducer::validate_file_path(path)?;
                self.selected_file = path;
                self.file_picker_requested = false;
                Ok("Datei ausgewählt".to_string())
            }
            AppAction::RequestFilePicker => {
                if self.file_picker_requested {
                    return Ok("Dateiauswahl ist bereits geöffnet".to_string());
                }
                self.file_picker_requested = true;
                Ok("Dateiauswahl geöffnet".to_string())
            }
            AppAction::CancelFileSelection => {
                self.file_picker_requested = false;
                Ok("Dateiauswahl abgebrochen".to_string())
            }
            AppAction::Reset => {
                self.input.clear();
                self.status = AppStatus::Ready;
                self.busy = false;
                self.error_message = None;
                self.focused_element = None;
                Ok("zurückgesetzt".to_string())
            }
            AppAction::OpenAbout => {
                self.about_open = true;
                self.screen = AppScreen::About;
                Ok("Info-Dialog geöffnet".to_string())
            }
            AppAction::CloseAbout => {
                self.about_open = false;
                self.screen = AppScreen::Main;
                Ok("Info-Dialog geschlossen".to_string())
            }
        }
    }

    fn apply_event(&mut self, event: &AppEvent) -> String {
        match event {
            AppEvent::SubmissionStarted => {
                self.busy = true;
                self.status = AppStatus::Busy;
                self.error_message = None;
                "wird ausgeführt".to_string()
            }
            AppEvent::SubmissionSucceeded => {
                self.busy = false;
                self.status = AppStatus::Success;
                self.error_message = None;
                "Befehl ausgeführt".to_string()
            }
            AppEvent::SubmissionFailed { message } => {
                self.busy = false;
                self.status = AppStatus::Error;
                self.error_message = Some(message.clone());
                message.clone()
            }
            AppEvent::ActionFailed { message } => {
                self.status = AppStatus::Error;
                self.error_message = Some(message.clone());
                message.clone()
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    SetInput { value: String },
    Focus { element_id: String },
    ClearFocus,
    SelectFile { path: String },
    RequestFilePicker,
    CancelFileSelection,
    Reset,
    OpenAbout,
    CloseAbout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    SubmissionStarted,
    SubmissionSucceeded,
    SubmissionFailed { message: String },
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

        let path = Path::new(trimmed);
        if path.is_dir() {
            return Err(ApplicationError::InvalidPayload(
                "Dateipfad muss auf eine Datei verweisen".into(),
            ));
        }

        Ok(trimmed.to_string())
    }
}
