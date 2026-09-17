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
    pub development_file_path: String,
    pub selected_file: String,
    pub status: AppStatus,
    pub busy: bool,
    pub about_open: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            revision: 0,
            screen: AppScreen::Main,
            input: String::new(),
            development_file_path: "/workspace/Cargo.toml".to_string(),
            selected_file: String::new(),
            status: AppStatus::Ready,
            busy: false,
            about_open: false,
        }
    }

    pub fn apply_action(&mut self, action: &AppAction) -> Result<String, ApplicationError> {
        match action {
            AppAction::SetInput { value } => {
                if value.len() > 4096 {
                    return Err(ApplicationError::InvalidPayload(
                        "Eingabe überschreitet 4096 Zeichen".into(),
                    ));
                }
                self.input = value.clone();
                Ok("Wert aktualisiert".to_string())
            }
            AppAction::SetDevelopmentFilePath { path } => {
                let path = AppReducer::validate_development_file_path(path)?;
                self.development_file_path = path;
                Ok("Entwicklungsdateipfad aktualisiert".to_string())
            }
            AppAction::SelectDevelopmentFile => {
                let path = self.development_file_path.trim().to_string();
                let path = AppReducer::validate_development_file_path(&path)?;
                self.selected_file = path;
                Ok("Datei ausgewählt".to_string())
            }
            AppAction::Reset => {
                self.input.clear();
                self.status = AppStatus::Ready;
                self.busy = false;
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
            AppAction::Submit => {
                self.busy = true;
                self.status = AppStatus::Busy;
                Ok("wird ausgeführt".to_string())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    SetInput { value: String },
    SetDevelopmentFilePath { path: String },
    SelectDevelopmentFile,
    Reset,
    Submit,
    OpenAbout,
    CloseAbout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub message: String,
    pub state: AppState,
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

pub struct AppReducer;

impl AppReducer {
    pub fn apply(state: &mut AppState, action: &AppAction) -> Result<String, ApplicationError> {
        state.apply_action(action)
    }

    pub fn validate_development_file_path(path: &str) -> Result<String, ApplicationError> {
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
}
