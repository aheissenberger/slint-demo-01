use domain::AppStatus;
use std::collections::HashMap;

/// Supported user-interface locales. German is the product default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Locale {
    #[default]
    German,
    English,
}

/// Small, dependency-free translation catalog for **dynamic, domain-derived**
/// text that Rust computes at runtime (the current [`AppStatus`] label, the
/// "Fehler: " error prefix that's spliced together with a domain error
/// message, and similar values a `.slint` file can't express as a literal).
///
/// Static UI strings (titles, menu items, form labels, dialog text, ...) are
/// **not** in this catalog. They are marked with Slint's own `@tr(...)`
/// translation macro directly in `ui/**/*.slint` and resolved through
/// Slint's built-in translation infrastructure (bundled `.po` message
/// catalogs under `translations/`, see `crates/slint-demo/build.rs`). This
/// keeps one localization mechanism per concern: Slint owns static
/// presentation text, `Catalog` owns text that depends on runtime domain
/// state.
#[derive(Debug, Clone)]
pub struct Catalog {
    locale: Locale,
    messages: HashMap<&'static str, &'static str>,
}

impl Catalog {
    pub fn new(locale: Locale) -> Self {
        let messages = match locale {
            Locale::German => [
                ("status.ready", "bereit"),
                ("status.busy", "wird ausgeführt"),
                ("status.success", "erfolgreich"),
                ("status.error", "Fehler"),
                ("status.error_prefix", "Fehler: "),
                ("action.cancel", "Abbrechen"),
            ],
            Locale::English => [
                ("status.ready", "ready"),
                ("status.busy", "running"),
                ("status.success", "successful"),
                ("status.error", "error"),
                ("status.error_prefix", "Error: "),
                ("action.cancel", "Cancel"),
            ],
        }
        .into_iter()
        .collect();
        Self { locale, messages }
    }

    pub fn locale(&self) -> Locale {
        self.locale
    }

    pub fn text<'a>(&'a self, key: &'a str) -> &'a str {
        self.messages.get(key).copied().unwrap_or(key)
    }

    pub fn status(&self, status: &AppStatus) -> &str {
        let key = match status {
            AppStatus::Ready => "status.ready",
            AppStatus::Busy => "status.busy",
            AppStatus::Success => "status.success",
            AppStatus::Error => "status.error",
        };
        self.text(key)
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new(Locale::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_is_the_default_and_unknown_keys_are_safe() {
        let catalog = Catalog::default();
        assert_eq!(catalog.locale(), Locale::German);
        assert_eq!(catalog.text("status.ready"), "bereit");
        assert_eq!(catalog.text("missing.key"), "missing.key");
    }

    #[test]
    fn additional_locale_can_be_selected_without_persistence_changes() {
        assert_eq!(
            Catalog::new(Locale::English).text("action.cancel"),
            "Cancel"
        );
    }

    #[test]
    fn status_labels_are_resolved_through_the_catalog() {
        assert_eq!(
            Catalog::default().status(&AppStatus::Busy),
            "wird ausgeführt"
        );
        assert_eq!(
            Catalog::new(Locale::English).status(&AppStatus::Busy),
            "running"
        );
    }

    #[test]
    fn error_prefix_is_resolved_through_the_catalog_per_locale() {
        assert_eq!(Catalog::default().text("status.error_prefix"), "Fehler: ");
        assert_eq!(
            Catalog::new(Locale::English).text("status.error_prefix"),
            "Error: "
        );
    }
}
