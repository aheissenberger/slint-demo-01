use domain::{AppScreen, AppStatus, DomainError};
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
    pub active_dialog: Option<ActiveDialog>,
    pub theme_mode: ThemeMode,
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
            active_dialog: None,
            theme_mode: ThemeMode::System,
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
            AppAction::Reset => {
                self.input.clear();
                self.status = AppStatus::Ready;
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

impl Default for AppState {
    fn default() -> Self {
        Self::new()
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
    Reset,
    OpenAbout,
    CloseAbout,
    OpenSettings,
    CloseSettings,
    SetThemeMode { mode: ThemeMode },
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
