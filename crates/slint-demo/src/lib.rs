#[cfg(feature = "agent-api")]
use agent_api::AgentApi;
use application::{AppAction, AppEffect, AppState, Catalog, StateUpdate};
use infrastructure::initialize_repository;
use runtime::{show_notification, NativeNotification, RuntimeStorage, WindowState};
use slint::SharedString;
use slint::{CloseRequestResponse, ComponentHandle, PhysicalPosition, PhysicalSize};
use std::{io, sync::Arc, thread, time::Instant};

slint::include_modules!();

mod runtime;

fn apply_state(ui: &MainWindow, state: AppState) {
    let catalog = Catalog::default();
    let status = state.error_message.as_deref().map_or_else(
        || catalog.status(&state.status).to_string(),
        |error| format!("{}{error}", catalog.text("status.error_prefix")),
    );
    ui.set_status(status.into());
    ui.set_submit_enabled(state.can_submit());
    ui.set_cancel_enabled(state.busy);
    ui.set_progress(state.progress.map_or(-1, i32::from));
    if ui.get_input_value().as_str() != state.input {
        ui.set_input_value(state.input.clone().into());
    }
    if ui.get_selected_file().as_str() != state.selected_file {
        ui.set_selected_file(state.selected_file.clone().into());
    }
    if ui.get_about_visible() != state.is_about_open() {
        if state.is_about_open() {
            ui.invoke_show_about();
        } else {
            ui.invoke_hide_about();
        }
    }
    if ui.get_settings_visible() != state.is_settings_open() {
        if state.is_settings_open() {
            ui.invoke_show_settings();
        } else {
            ui.invoke_hide_settings();
        }
    }
    if ui.get_theme_mode().as_str() != state.theme_mode.as_str() {
        ui.set_theme_mode(state.theme_mode.as_str().into());
    }
}

pub struct DesktopApp {
    store: Arc<application::AppStateStore<infrastructure::FileRepository>>,
    #[cfg(feature = "agent-api")]
    agent_api: AgentApi<infrastructure::FileRepository>,
}

impl Default for DesktopApp {
    fn default() -> Self {
        let repository = initialize_repository();
        let store = Arc::new(application::AppStateStore::new(repository.build_service()));
        Self {
            #[cfg(feature = "agent-api")]
            agent_api: AgentApi::from_store(Arc::clone(&store)),
            store,
        }
    }
}

impl DesktopApp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run() -> Result<(), DesktopAppError> {
        let startup_started = Instant::now();
        let runtime_storage = Arc::new(RuntimeStorage::open().map_err(DesktopAppError::Runtime)?);
        let _single_instance = runtime_storage
            .acquire_single_instance()
            .map_err(DesktopAppError::Runtime)?;
        let app = Self::new();
        let ui = MainWindow::new().map_err(|error| DesktopAppError::Ui(error.to_string()))?;
        ui.set_app_version(env!("CARGO_PKG_VERSION").into());
        Self::restore_window_state(&ui, &runtime_storage);
        Self::configure_store(&ui, &app.store);
        Self::configure_orderly_shutdown(&ui, &app.store, Arc::clone(&runtime_storage));
        #[cfg(feature = "agent-api")]
        Self::start_agent_api(&app);
        tracing::info!(
            startup_ms = startup_started.elapsed().as_secs_f64() * 1000.0,
            "Anwendungsstart abgeschlossen"
        );
        ui.run()
            .map_err(|error| DesktopAppError::Ui(error.to_string()))
    }

    #[cfg(feature = "agent-api")]
    fn start_agent_api(app: &Self) {
        let api = app.agent_api.clone();
        thread::spawn(move || {
            if let Err(error) = api.serve("127.0.0.1:8080") {
                tracing::error!(%error, "agent API stopped");
            }
        });
    }

    fn configure_store(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::FileRepository>>,
    ) {
        let store = Arc::clone(store);
        let updates = store.subscribe().expect("state subscription");
        apply_state(
            ui,
            store.current_state().expect("initial application state"),
        );
        Self::start_state_sync(ui, updates);
        Self::configure_input(ui, &store);
        Self::configure_reset(ui, &store);
        Self::configure_file_picker(ui, &store);
        Self::configure_native_file_picker(ui, &store);
        Self::configure_about(ui, &store);
        Self::configure_settings(ui, &store);
        Self::configure_theme_mode(ui, &store);
        Self::configure_submit(ui, &store);
        Self::configure_cancel(ui, &store);
    }

    fn start_state_sync(ui: &MainWindow, updates: std::sync::mpsc::Receiver<StateUpdate>) {
        let weak = ui.as_weak();
        thread::spawn(move || {
            let mut was_busy = false;
            while let Ok(update) = updates.recv() {
                if let Some(notification) = notification_for_completion(was_busy, &update.state) {
                    show_notification(&notification);
                }
                was_busy = update.state.busy;
                if weak
                    .upgrade_in_event_loop(move |ui| {
                        apply_state(&ui, update.state);
                        match update.effect {
                            Some(AppEffect::Focus { element_id }) => {
                                ui.invoke_focus_control(element_id.into());
                            }
                            Some(AppEffect::OpenNativeFilePicker) => {
                                ui.invoke_show_native_file_picker();
                            }
                            None => {}
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    fn configure_input(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::FileRepository>>,
    ) {
        let store = Arc::clone(store);
        ui.on_input_changed(move |value| {
            if let Err(error) = store.dispatch(AppAction::SetInput {
                value: value.to_string(),
            }) {
                tracing::error!(%error, "failed to synchronize input");
            }
        });
    }

    fn configure_reset(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::FileRepository>>,
    ) {
        let store = Arc::clone(store);
        ui.on_reset(move || {
            if let Err(error) = store.dispatch(AppAction::Reset) {
                tracing::error!(%error, "failed to reset application");
            }
        });
    }

    fn configure_about(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::FileRepository>>,
    ) {
        let store = Arc::clone(store);
        ui.on_about_visibility_changed(move |open| {
            let action = if open {
                AppAction::OpenAbout
            } else {
                AppAction::CloseAbout
            };
            if let Err(error) = store.dispatch(action) {
                tracing::error!(%error, "failed to synchronize about dialog");
            }
        });
    }

    fn configure_settings(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::FileRepository>>,
    ) {
        let store = Arc::clone(store);
        ui.on_settings_visibility_changed(move |open| {
            let action = if open {
                AppAction::OpenSettings
            } else {
                AppAction::CloseSettings
            };
            if let Err(error) = store.dispatch(action) {
                tracing::error!(%error, "failed to synchronize settings dialog");
            }
        });
    }

    fn configure_theme_mode(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::FileRepository>>,
    ) {
        let store = Arc::clone(store);
        ui.on_theme_mode_changed(move |mode| {
            let mode = match application::ThemeMode::parse(mode.as_str()) {
                Ok(mode) => mode,
                Err(error) => {
                    tracing::error!(%error, "failed to parse theme mode");
                    return;
                }
            };
            if let Err(error) = store.dispatch(AppAction::SetThemeMode { mode }) {
                tracing::error!(%error, "failed to synchronize theme mode");
            }
        });
    }

    fn configure_submit(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::FileRepository>>,
    ) {
        let store = Arc::clone(store);
        ui.on_submit(move |value: SharedString| {
            let _ = value;
            if let Err(error) = store.submit() {
                tracing::error!(%error, "failed to submit value");
            }
        });
    }

    fn configure_cancel(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::FileRepository>>,
    ) {
        let store = Arc::clone(store);
        ui.on_cancel(move || {
            if let Err(error) = store.cancel_submission() {
                tracing::error!(%error, "Vorgang konnte nicht abgebrochen werden");
            }
        });
    }

    fn configure_file_picker(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::FileRepository>>,
    ) {
        let store = Arc::clone(store);
        ui.on_pick_file(move || {
            if let Err(error) = store.dispatch(AppAction::RequestFilePicker) {
                tracing::error!(%error, "failed to request file selection");
            }
        });
    }

    fn configure_native_file_picker(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::FileRepository>>,
    ) {
        let store = Arc::clone(store);
        ui.on_show_native_file_picker(move || {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                if let Err(error) = store.dispatch(AppAction::SelectFile {
                    path: path.to_string_lossy().into_owned(),
                }) {
                    tracing::error!(%error, "failed to select file");
                }
            } else if let Err(error) = store.dispatch(AppAction::CancelFileSelection) {
                tracing::error!(%error, "failed to cancel file selection");
            }
        });
    }

    fn restore_window_state(ui: &MainWindow, runtime_storage: &RuntimeStorage) {
        match runtime_storage.load_window_state() {
            Ok(Some(state)) => {
                ui.window()
                    .set_size(PhysicalSize::new(state.width, state.height));
                ui.window()
                    .set_position(PhysicalPosition::new(state.x, state.y));
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, "Fensterzustand konnte nicht geladen werden");
            }
        }
    }

    fn configure_orderly_shutdown(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::FileRepository>>,
        runtime_storage: Arc<RuntimeStorage>,
    ) {
        let weak = ui.as_weak();
        let store = Arc::clone(store);
        ui.window().on_close_requested(move || {
            if let Some(ui) = weak.upgrade() {
                let size = ui.window().size();
                let position = ui.window().position();
                if let Err(error) = runtime_storage.save_window_state(WindowState {
                    width: size.width,
                    height: size.height,
                    x: position.x,
                    y: position.y,
                }) {
                    tracing::error!(%error, "Fensterzustand konnte nicht gespeichert werden");
                }
            }
            if let Err(error) = store.cancel_submission() {
                tracing::error!(%error, "Laufender Vorgang konnte beim Beenden nicht abgebrochen werden");
            }
            CloseRequestResponse::HideWindow
        });
    }
}

#[derive(Debug)]
pub enum DesktopAppError {
    Runtime(io::Error),
    Ui(String),
}

impl DesktopAppError {
    pub fn is_already_running(&self) -> bool {
        matches!(self, Self::Runtime(error) if runtime::is_single_instance_error(error))
    }
}

impl std::fmt::Display for DesktopAppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Runtime(error) if runtime::is_single_instance_error(error) => {
                formatter.write_str("Die Anwendung wird bereits ausgeführt")
            }
            Self::Runtime(error) => {
                write!(
                    formatter,
                    "Laufzeitumgebung konnte nicht vorbereitet werden: {error}"
                )
            }
            Self::Ui(error) => {
                write!(
                    formatter,
                    "Benutzeroberfläche konnte nicht gestartet werden: {error}"
                )
            }
        }
    }
}

impl std::error::Error for DesktopAppError {}

fn notification_for_completion(was_busy: bool, state: &AppState) -> Option<NativeNotification> {
    if !was_busy || state.busy {
        return None;
    }
    match state.status {
        domain::AppStatus::Success => Some(NativeNotification {
            title: "Übermittlung abgeschlossen".into(),
            body: "Der Vorgang wurde erfolgreich abgeschlossen.".into(),
        }),
        domain::AppStatus::Error => Some(NativeNotification {
            title: "Übermittlung fehlgeschlagen".into(),
            body: state
                .error_message
                .clone()
                .unwrap_or_else(|| "Der Vorgang konnte nicht abgeschlossen werden.".into()),
        }),
        domain::AppStatus::Ready | domain::AppStatus::Busy => None,
    }
}

/// Coverage tests for the bundled Slint `@tr(...)` translations. Static UI
/// text lives in `ui/**/*.slint` and is extracted into
/// `translations/slint-demo.pot`; the German catalog is the identity
/// translation of the source strings, and the English catalog carries real
/// translations. These tests parse the checked-in `.pot`/`.po` files
/// directly (no external `slint-tr-extractor` binary required) to prove
/// every extracted string has both a German and an English translation, and
/// that German entries have not silently drifted from the `@tr(...)`
/// source text.
#[cfg(test)]
mod translation_coverage_tests {
    use std::collections::BTreeMap;

    type TranslationKey = (String, String);

    /// Parses a gettext `.po`/`.pot` file into `(msgctxt, msgid) -> msgstr`
    /// entries, joining multi-line quoted strings and skipping the header
    /// entry (whose `msgid` is the empty string).
    fn parse_po(source: &str) -> BTreeMap<TranslationKey, String> {
        fn unquote(line: &str) -> String {
            let trimmed = line.trim();
            let inner = trimmed
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(trimmed);
            inner.replace("\\\"", "\"").replace("\\\\", "\\")
        }

        let mut entries = BTreeMap::new();
        let mut ctx = String::new();
        let mut collecting: Option<&str> = None;
        let mut msgid = String::new();
        let mut msgstr = String::new();

        for raw_line in source.lines() {
            let line = raw_line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("msgctxt ") {
                if !msgid.is_empty() || !msgstr.is_empty() {
                    if !msgid.is_empty() {
                        entries.insert((ctx.clone(), msgid.clone()), msgstr.clone());
                    }
                    msgid.clear();
                    msgstr.clear();
                }
                ctx = unquote(rest);
                collecting = None;
            } else if let Some(rest) = line.strip_prefix("msgid ") {
                if !msgid.is_empty() {
                    entries.insert((ctx.clone(), msgid.clone()), msgstr.clone());
                    msgstr.clear();
                }
                msgid = unquote(rest);
                collecting = Some("msgid");
            } else if let Some(rest) = line.strip_prefix("msgstr ") {
                msgstr = unquote(rest);
                collecting = Some("msgstr");
            } else if line.starts_with('"') {
                match collecting {
                    Some("msgid") => msgid.push_str(&unquote(line)),
                    Some("msgstr") => msgstr.push_str(&unquote(line)),
                    _ => {}
                }
            }
        }
        if !msgid.is_empty() {
            entries.insert((ctx, msgid), msgstr);
        }
        // Drop the header entry, whose msgid is the empty string.
        entries.retain(|(_, msgid), _| !msgid.is_empty());
        entries
    }

    const POT: &str = include_str!("../translations/slint-demo.pot");
    const DE_PO: &str = include_str!("../translations/de/LC_MESSAGES/slint-demo.po");
    const EN_PO: &str = include_str!("../translations/en/LC_MESSAGES/slint-demo.po");

    #[test]
    fn extracted_template_has_a_translation_for_every_locale() {
        let template = parse_po(POT);
        let de = parse_po(DE_PO);
        let en = parse_po(EN_PO);

        assert!(
            template.len() >= 25,
            "expected the extracted .pot template to contain the full set of static UI              strings, found only {}",
            template.len()
        );

        for key in template.keys() {
            assert!(
                de.contains_key(key),
                "missing German translation for {key:?}; regenerate slint-demo.po with                  slint-tr-extractor"
            );
            assert!(
                en.contains_key(key),
                "missing English translation for {key:?}; add it to                  translations/en/LC_MESSAGES/slint-demo.po"
            );
        }
    }

    #[test]
    fn german_translations_are_the_identity_of_the_tr_source_text() {
        let template = parse_po(POT);
        let de = parse_po(DE_PO);

        for key in template.keys() {
            let (ctx, msgid) = key;
            let translated = de
                .get(key)
                .unwrap_or_else(|| panic!("missing German translation for {key:?}"));
            assert_eq!(
                translated, msgid,
                "German is the @tr(...) source language, so context {ctx:?} entry                  {msgid:?} must be its own identity translation"
            );
        }
    }

    #[test]
    fn english_translations_are_not_left_blank() {
        let template = parse_po(POT);
        let en = parse_po(EN_PO);

        for key in template.keys() {
            let translated = en
                .get(key)
                .unwrap_or_else(|| panic!("missing English translation for {key:?}"));
            assert!(
                !translated.is_empty(),
                "English translation for {key:?} must not be an empty msgstr"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_submissions_emit_native_notifications() {
        let mut state = AppState::new();
        state.status = domain::AppStatus::Success;
        assert_eq!(
            notification_for_completion(true, &state),
            Some(NativeNotification {
                title: "Übermittlung abgeschlossen".into(),
                body: "Der Vorgang wurde erfolgreich abgeschlossen.".into(),
            })
        );
    }

    #[test]
    fn cancelled_or_unrelated_updates_do_not_emit_notifications() {
        let state = AppState::new();
        assert_eq!(notification_for_completion(true, &state), None);
        assert_eq!(notification_for_completion(false, &state), None);
    }
}
