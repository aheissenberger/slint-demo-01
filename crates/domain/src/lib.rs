use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppId(String);

impl AppId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::InvalidAppId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionValue(String);

impl SubmissionValue {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into().trim().to_string();
        if value.is_empty() {
            return Err(DomainError::EmptyValue);
        }
        if value.len() > 4096 {
            return Err(DomainError::ValueTooLong);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub app_name: String,
    pub server_url: String,
    pub enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            app_name: "Slint Agent Demo".to_string(),
            server_url: "https://example.internal".to_string(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionRecord {
    pub id: AppId,
    pub title: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub active: bool,
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("ungültige Anwendungskennung")]
    InvalidAppId,
    #[error("Vorgang ist im aktuellen Zustand nicht erlaubt")]
    InvalidState,
    #[error("Wert darf nicht leer sein")]
    EmptyValue,
    #[error("Wert überschreitet 4096 Zeichen")]
    ValueTooLong,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppStatus {
    #[default]
    Ready,
    Busy,
    Success,
    Error,
}

impl fmt::Display for AppStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Ready => "bereit",
            Self::Busy => "wird ausgeführt",
            Self::Success => "erfolgreich",
            Self::Error => "Fehler",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppScreen {
    #[default]
    Main,
    About,
    Settings,
}

impl fmt::Display for AppScreen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Main => "main",
            Self::About => "about",
            Self::Settings => "settings",
        };
        f.write_str(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_id_rejects_blank_values() {
        assert_eq!(
            AppId::new("  ").unwrap_err().to_string(),
            "ungültige Anwendungskennung"
        );
        assert_eq!(AppId::new("demo").unwrap().as_str(), "demo");
    }

    #[test]
    fn submission_value_is_trimmed_and_validated() {
        assert_eq!(SubmissionValue::new(" Test ").unwrap().as_str(), "Test");
        assert!(matches!(
            SubmissionValue::new(" "),
            Err(DomainError::EmptyValue)
        ));
    }

    #[test]
    fn default_settings_are_enabled() {
        let settings = AppSettings::default();
        assert!(settings.enabled);
        assert!(!settings.app_name.is_empty());
    }

    #[test]
    fn app_status_has_explicit_domain_values() {
        assert_eq!(AppStatus::Ready.to_string(), "bereit");
        assert_eq!(AppStatus::Busy.to_string(), "wird ausgeführt");
        assert_eq!(AppStatus::Success.to_string(), "erfolgreich");
        assert_eq!(AppStatus::Error.to_string(), "Fehler");
        assert_eq!(AppScreen::Main.to_string(), "main");
        assert_eq!(AppScreen::About.to_string(), "about");
        assert_eq!(AppScreen::Settings.to_string(), "settings");
    }
}
