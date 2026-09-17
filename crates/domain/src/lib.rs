use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
pub struct ExampleSettings {
    pub app_name: String,
    pub server_url: String,
    pub enabled: bool,
}

impl Default for ExampleSettings {
    fn default() -> Self {
        Self {
            app_name: "Slint Agent Demo".to_string(),
            server_url: "https://example.internal".to_string(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExampleRecord {
    pub id: AppId,
    pub title: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub active: bool,
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid app identifier")]
    InvalidAppId,
    #[error("operation is not allowed in the current state")]
    InvalidState,
    #[error("value must be non-empty")]
    EmptyValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppStateSnapshot {
    pub screen: String,
    pub status: String,
    pub busy: bool,
    pub last_updated: DateTime<Utc>,
}

impl AppStateSnapshot {
    pub fn ready() -> Self {
        Self {
            screen: "main".to_string(),
            status: "ready".to_string(),
            busy: false,
            last_updated: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_id_rejects_blank_values() {
        assert_eq!(
            AppId::new("  ").unwrap_err().to_string(),
            "invalid app identifier"
        );
        assert_eq!(AppId::new("demo").unwrap().as_str(), "demo");
    }

    #[test]
    fn default_settings_are_enabled() {
        let settings = ExampleSettings::default();
        assert!(settings.enabled);
        assert!(!settings.app_name.is_empty());
    }
}
