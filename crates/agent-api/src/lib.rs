use application::{
    AppAction, AppService, AppStateStore, ApplicationError, FieldValidation, NoteListItem,
    TaskState, ThemeMode, UiError,
};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::{info, instrument, trace};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStateResponse {
    pub screen: String,
    pub status: String,
    pub busy: bool,
    pub progress: Option<u8>,
    pub error: Option<String>,
    pub ui_error: Option<UiError>,
    pub validation_errors: Vec<FieldValidation>,
    pub tasks: Vec<TaskState>,
    pub active_task_id: Option<String>,
    pub can_retry: bool,
    pub theme_mode: String,
    pub notes: Vec<NoteListItem>,
    pub selected_note_id: Option<String>,
    pub note_title: String,
    pub note_body: String,
    pub data_summary: String,
    pub last_backup_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentElement {
    pub id: String,
    pub role: String,
    pub actions: Vec<String>,
    pub enabled: bool,
    pub value: Option<String>,
    pub accessible_label: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct UiComponentMetadata {
    pub role: &'static str,
    pub actions: &'static [&'static str],
}

const UI_COMPONENT_METADATA: &[(&str, UiComponentMetadata)] = &[
    (
        "main.input",
        UiComponentMetadata {
            role: "textbox",
            actions: &["get_value", "set_value", "focus"],
        },
    ),
    (
        "main.submit",
        UiComponentMetadata {
            role: "button",
            actions: &["click", "focus"],
        },
    ),
    (
        "main.cancel",
        UiComponentMetadata {
            role: "button",
            actions: &["click", "focus"],
        },
    ),
    (
        "main.reset",
        UiComponentMetadata {
            role: "button",
            actions: &["click", "focus"],
        },
    ),
    (
        "main.file-picker",
        UiComponentMetadata {
            role: "button",
            actions: &["click", "get_value", "set_value", "focus"],
        },
    ),
    (
        "main.selected-file",
        UiComponentMetadata {
            role: "status",
            actions: &["get_value"],
        },
    ),
    (
        "main.status",
        UiComponentMetadata {
            role: "status",
            actions: &[],
        },
    ),
    (
        "main.error",
        UiComponentMetadata {
            role: "alert",
            actions: &["get_value"],
        },
    ),
    (
        "main.error.dismiss",
        UiComponentMetadata {
            role: "button",
            actions: &["click", "focus"],
        },
    ),
    (
        "main.retry",
        UiComponentMetadata {
            role: "button",
            actions: &["click", "focus"],
        },
    ),
    (
        "main.tasks",
        UiComponentMetadata {
            role: "status",
            actions: &["get_value"],
        },
    ),
    (
        "notes.list",
        UiComponentMetadata {
            role: "list",
            actions: &["get_value"],
        },
    ),
    (
        "notes.new",
        UiComponentMetadata {
            role: "button",
            actions: &["click", "focus"],
        },
    ),
    (
        "notes.title",
        UiComponentMetadata {
            role: "textbox",
            actions: &["get_value", "set_value", "focus"],
        },
    ),
    (
        "notes.body",
        UiComponentMetadata {
            role: "textbox",
            actions: &["get_value", "set_value", "focus"],
        },
    ),
    (
        "notes.save",
        UiComponentMetadata {
            role: "button",
            actions: &["click", "focus"],
        },
    ),
    (
        "notes.archive",
        UiComponentMetadata {
            role: "button",
            actions: &["click", "focus"],
        },
    ),
    (
        "notes.delete",
        UiComponentMetadata {
            role: "button",
            actions: &["click", "focus"],
        },
    ),
    (
        "notes.item.0",
        UiComponentMetadata {
            role: "button",
            actions: &["click"],
        },
    ),
    (
        "notes.item.1",
        UiComponentMetadata {
            role: "button",
            actions: &["click"],
        },
    ),
    (
        "notes.item.2",
        UiComponentMetadata {
            role: "button",
            actions: &["click"],
        },
    ),
    (
        "notes.item.3",
        UiComponentMetadata {
            role: "button",
            actions: &["click"],
        },
    ),
    (
        "notes.item.4",
        UiComponentMetadata {
            role: "button",
            actions: &["click"],
        },
    ),
    (
        "file.settings",
        UiComponentMetadata {
            role: "menuitem",
            actions: &["click"],
        },
    ),
    (
        "help.about",
        UiComponentMetadata {
            role: "menuitem",
            actions: &["click"],
        },
    ),
    (
        "settings.dialog",
        UiComponentMetadata {
            role: "dialog",
            actions: &[],
        },
    ),
    (
        "settings.theme.system",
        UiComponentMetadata {
            role: "radio",
            actions: &["click"],
        },
    ),
    (
        "settings.theme.light",
        UiComponentMetadata {
            role: "radio",
            actions: &["click"],
        },
    ),
    (
        "settings.theme.dark",
        UiComponentMetadata {
            role: "radio",
            actions: &["click"],
        },
    ),
    (
        "settings.close",
        UiComponentMetadata {
            role: "button",
            actions: &["click"],
        },
    ),
    (
        "settings.data.summary",
        UiComponentMetadata {
            role: "status",
            actions: &["get_value"],
        },
    ),
    (
        "settings.data.backup-path",
        UiComponentMetadata {
            role: "status",
            actions: &["get_value"],
        },
    ),
    (
        "settings.data.backup",
        UiComponentMetadata {
            role: "button",
            actions: &["click"],
        },
    ),
    (
        "settings.data.reset",
        UiComponentMetadata {
            role: "button",
            actions: &["click"],
        },
    ),
    (
        "about.dialog",
        UiComponentMetadata {
            role: "dialog",
            actions: &[],
        },
    ),
    (
        "about.close",
        UiComponentMetadata {
            role: "button",
            actions: &["click"],
        },
    ),
    (
        "error.dialog",
        UiComponentMetadata {
            role: "alertdialog",
            actions: &[],
        },
    ),
    (
        "error.dialog.close",
        UiComponentMetadata {
            role: "button",
            actions: &["click", "focus"],
        },
    ),
];

pub fn ui_component_metadata(id: &str) -> Option<UiComponentMetadata> {
    UI_COMPONENT_METADATA
        .iter()
        .find_map(|(candidate, metadata)| (*candidate == id).then_some(*metadata))
}

fn element(id: &str, enabled: bool, value: Option<String>, label: &str) -> AgentElement {
    let metadata = ui_component_metadata(id).expect("agent element metadata");
    AgentElement {
        id: id.to_string(),
        role: metadata.role.to_string(),
        actions: metadata
            .actions
            .iter()
            .map(|action| (*action).to_string())
            .collect(),
        enabled,
        value,
        accessible_label: Some(label.to_string()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiInspectionResponse {
    pub screen: String,
    pub elements: Vec<AgentElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandRequest {
    pub command: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActionRequest {
    pub action: String,
    pub id: String,
    pub value: Option<String>,
}

/// Transport-independent semantic interface exposed by the agent service.
///
/// HTTP, MCP, and future transports should depend on this contract rather
/// than on the concrete application state implementation.
pub trait AgentService: Send {
    fn get_state(&self) -> Result<AgentStateResponse, ApplicationError>;
    fn inspect_ui(&self) -> Result<UiInspectionResponse, ApplicationError>;
    fn execute_command(&self, request: AgentCommandRequest) -> Result<String, ApplicationError>;
    fn execute_ui_action(&self, action: AgentActionRequest) -> Result<String, ApplicationError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentErrorResponse {
    pub error: String,
    pub code: String,
    pub user_message: String,
    pub diagnostic_message: String,
    pub recoverable: bool,
    pub element: Option<String>,
}

#[derive(Clone)]
pub struct AgentApi<S> {
    store: Arc<AppStateStore<S>>,
}

impl<S> AgentApi<S>
where
    S: application::AppRepository
        + application::NoteRepository
        + application::DataMaintenanceRepository
        + Clone
        + Send
        + Sync
        + 'static,
{
    pub fn new(service: AppService<S>) -> Self {
        Self::from_store(Arc::new(AppStateStore::new(service)))
    }

    pub fn from_store(store: Arc<AppStateStore<S>>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> Arc<AppStateStore<S>> {
        Arc::clone(&self.store)
    }

    #[instrument(name = "agent.get_state", skip(self), fields(component = "agent-api"))]
    pub fn get_state(&self) -> Result<AgentStateResponse, ApplicationError> {
        let state = self.store.current_state()?;
        Ok(AgentStateResponse {
            screen: state.screen().to_string(),
            status: state.status.to_string(),
            busy: state.busy,
            progress: state.progress,
            error: state.error_message.clone(),
            ui_error: state.ui_error.clone(),
            validation_errors: state.validation_errors.clone(),
            tasks: state.tasks.clone(),
            active_task_id: state.active_task_id.clone(),
            can_retry: state.can_retry(),
            theme_mode: state.theme_mode.as_str().to_string(),
            notes: state.notes,
            selected_note_id: state.selected_note_id,
            note_title: state.note_title,
            note_body: state.note_body,
            data_summary: state.data_summary,
            last_backup_path: state.last_backup_path,
        })
    }

    #[instrument(name = "agent.inspect_ui", skip(self), fields(component = "agent-api"))]
    pub fn inspect_ui(&self) -> Result<UiInspectionResponse, ApplicationError> {
        let state = self.store.current_state()?;
        let main_controls_enabled = state.active_dialog.is_none();
        let mut elements = main_elements(&state, main_controls_enabled);
        elements.extend(note_elements(&state, main_controls_enabled));
        elements.extend(shell_elements(main_controls_enabled));
        if state.is_settings_open() {
            elements.extend(settings_elements(&state));
        }
        if state.is_about_open() {
            elements.extend(about_elements());
        }
        if let Some(error) = state.critical_error() {
            elements.extend(error_dialog_elements(error));
        }

        Ok(UiInspectionResponse {
            screen: state.screen().to_string(),
            elements,
        })
    }

    pub fn revision(&self) -> Result<u64, ApplicationError> {
        Ok(self.store.current_state()?.revision)
    }

    #[instrument(
        name = "agent.execute_command",
        skip(self),
        fields(component = "agent-api")
    )]
    pub fn execute_command(
        &self,
        request: AgentCommandRequest,
    ) -> Result<String, ApplicationError> {
        trace!(command = %request.command, "received command");
        let command = match request.command.as_str() {
            "submit" => return Ok(self.store.submit()?.message),
            "cancel" => return Ok(self.store.cancel_submission()?.message),
            "cancel_task" => {
                let task_id = required_argument(&request.arguments, "id")?;
                return Ok(self.store.cancel_task(task_id)?.message);
            }
            "retry_last_task" => AppAction::RetryLastFailedTask,
            "dismiss_error" => AppAction::DismissError,
            "create_data_backup" => AppAction::CreateDataBackup,
            "reset_user_data" => AppAction::ResetUserData,
            "reset" => AppAction::Reset,
            "open_about" => AppAction::OpenAbout,
            "close_about" => AppAction::CloseAbout,
            "open_settings" => AppAction::OpenSettings,
            "close_settings" => AppAction::CloseSettings,
            "set_theme" => {
                let mode = required_argument(&request.arguments, "mode")?;
                AppAction::SetThemeMode {
                    mode: ThemeMode::parse(mode)?,
                }
            }
            "set_input" => {
                let value = required_argument(&request.arguments, "value")?.to_string();
                AppAction::SetInput { value }
            }
            "select_file" => {
                let value = required_argument(&request.arguments, "path")?.to_string();
                AppAction::SelectFile { path: value }
            }
            "new_note" => AppAction::StartNewNote,
            "select_note" => AppAction::SelectNote {
                id: required_argument(&request.arguments, "id")?.to_string(),
            },
            "set_note_title" => AppAction::SetNoteTitle {
                value: required_argument(&request.arguments, "value")?.to_string(),
            },
            "set_note_body" => AppAction::SetNoteBody {
                value: required_argument(&request.arguments, "value")?.to_string(),
            },
            "save_note" => AppAction::SaveNote,
            "archive_note" => AppAction::ArchiveNote,
            "delete_note" => AppAction::DeleteNote,
            _ => {
                return Err(ApplicationError::InvalidPayload(format!(
                    "nicht unterstützter Befehl: {}",
                    request.command
                )))
            }
        };
        Ok(self.store.dispatch(command)?.message)
    }

    #[instrument(
        name = "agent.execute_ui_action",
        skip(self),
        fields(component = "agent-api")
    )]
    pub fn execute_ui_action(
        &self,
        action: AgentActionRequest,
    ) -> Result<String, ApplicationError> {
        self.ensure_action_is_enabled(&action)?;
        match action.action.as_str() {
            "click" => self.click_ui_element(action),
            "set_value" => self.set_ui_value(action),
            "get_value" => self.get_ui_value(action),
            "focus" => self.focus_ui_element(action),
            _ => Err(unsupported_action(&action)),
        }
    }

    fn click_ui_element(&self, action: AgentActionRequest) -> Result<String, ApplicationError> {
        match action.id.as_str() {
            "main.submit" => Ok(self.store.submit()?.message),
            "main.cancel" => Ok(self.store.cancel_submission()?.message),
            "main.reset" => Ok(self.store.dispatch(AppAction::Reset)?.message),
            "main.retry" => Ok(self.store.dispatch(AppAction::RetryLastFailedTask)?.message),
            "main.error.dismiss" | "error.dialog.close" => {
                Ok(self.store.dispatch(AppAction::DismissError)?.message)
            }
            "main.file-picker" => Ok(self.store.dispatch(AppAction::RequestFilePicker)?.message),
            "help.about" => Ok(self.store.dispatch(AppAction::OpenAbout)?.message),
            "file.settings" => Ok(self.store.dispatch(AppAction::OpenSettings)?.message),
            "about.close" => Ok(self.store.dispatch(AppAction::CloseAbout)?.message),
            "settings.close" => Ok(self.store.dispatch(AppAction::CloseSettings)?.message),
            "settings.data.backup" => Ok(self.store.dispatch(AppAction::CreateDataBackup)?.message),
            "settings.data.reset" => Ok(self.store.dispatch(AppAction::ResetUserData)?.message),
            "settings.theme.system" => self.set_theme_mode(ThemeMode::System),
            "settings.theme.light" => self.set_theme_mode(ThemeMode::Light),
            "settings.theme.dark" => self.set_theme_mode(ThemeMode::Dark),
            "notes.new" => Ok(self.store.dispatch(AppAction::StartNewNote)?.message),
            "notes.save" => Ok(self.store.dispatch(AppAction::SaveNote)?.message),
            "notes.archive" => Ok(self.store.dispatch(AppAction::ArchiveNote)?.message),
            "notes.delete" => Ok(self.store.dispatch(AppAction::DeleteNote)?.message),
            id if id.starts_with("notes.item.") => self.select_note_slot(id),
            _ => Err(unsupported_action(&action)),
        }
    }

    fn set_ui_value(&self, action: AgentActionRequest) -> Result<String, ApplicationError> {
        match action.id.as_str() {
            "main.input" => Ok(self
                .store
                .dispatch(AppAction::SetInput {
                    value: required_action_value(&action)?.to_string(),
                })?
                .message),
            "main.file-picker" => Ok(self
                .store
                .dispatch(AppAction::SelectFile {
                    path: required_action_value(&action)?.to_string(),
                })?
                .message),
            "notes.title" => Ok(self
                .store
                .dispatch(AppAction::SetNoteTitle {
                    value: required_action_value(&action)?.to_string(),
                })?
                .message),
            "notes.body" => Ok(self
                .store
                .dispatch(AppAction::SetNoteBody {
                    value: required_action_value(&action)?.to_string(),
                })?
                .message),
            _ => Err(unsupported_action(&action)),
        }
    }

    fn get_ui_value(&self, action: AgentActionRequest) -> Result<String, ApplicationError> {
        let state = self.store.current_state()?;
        match action.id.as_str() {
            "main.file-picker" | "main.selected-file" => Ok(state.selected_file),
            "main.input" => Ok(state.input),
            "main.error" => state
                .visible_error()
                .map(error_value)
                .ok_or_else(|| ApplicationError::InvalidPayload("kein Fehler sichtbar".into())),
            "main.tasks" => Ok(task_summary(&state)),
            "settings.data.summary" => Ok(state.data_summary),
            "settings.data.backup-path" => Ok(state.last_backup_path.unwrap_or_default()),
            "notes.list" => Ok(state.notes.len().to_string()),
            "notes.title" => Ok(state.note_title),
            "notes.body" => Ok(state.note_body),
            _ => Err(unsupported_action(&action)),
        }
    }

    fn focus_ui_element(&self, action: AgentActionRequest) -> Result<String, ApplicationError> {
        match action.id.as_str() {
            "main.input" | "main.submit" | "main.reset" | "main.file-picker" | "main.cancel"
            | "main.retry" | "main.error.dismiss" | "error.dialog.close" | "notes.new"
            | "notes.title" | "notes.body" | "notes.save" | "notes.archive" | "notes.delete" => {
                Ok(self
                    .store
                    .dispatch(AppAction::Focus {
                        element_id: action.id,
                    })?
                    .message)
            }
            _ => Err(unsupported_action(&action)),
        }
    }

    fn set_theme_mode(&self, mode: ThemeMode) -> Result<String, ApplicationError> {
        Ok(self
            .store
            .dispatch(AppAction::SetThemeMode { mode })?
            .message)
    }

    fn select_note_slot(&self, id: &str) -> Result<String, ApplicationError> {
        let index = note_index_from_element_id(id)?;
        let note_id = self
            .store
            .current_state()?
            .notes
            .get(index)
            .map(|note| note.id.clone())
            .ok_or_else(|| {
                ApplicationError::InvalidPayload(format!("Element ist nicht sichtbar: {id}"))
            })?;
        Ok(self
            .store
            .dispatch(AppAction::SelectNote { id: note_id })?
            .message)
    }

    fn ensure_action_is_enabled(
        &self,
        action: &AgentActionRequest,
    ) -> Result<(), ApplicationError> {
        if !matches!(action.action.as_str(), "click" | "set_value" | "focus") {
            return Ok(());
        }

        let state = self.store.current_state()?;
        if state.active_dialog.is_some()
            && matches!(
                action.id.as_str(),
                "main.input"
                    | "main.submit"
                    | "main.cancel"
                    | "main.retry"
                    | "main.error.dismiss"
                    | "main.reset"
                    | "main.file-picker"
                    | "file.settings"
                    | "help.about"
                    | "notes.new"
                    | "notes.title"
                    | "notes.body"
                    | "notes.save"
                    | "notes.archive"
                    | "notes.delete"
            )
        {
            return Err(ApplicationError::InvalidPayload(format!(
                "Element ist deaktiviert: {}",
                action.id
            )));
        }
        if state.active_dialog.is_some() && action.id.starts_with("notes.item.") {
            return Err(ApplicationError::InvalidPayload(format!(
                "Element ist deaktiviert: {}",
                action.id
            )));
        }
        if action.id == "about.close" && !state.is_about_open() {
            return Err(ApplicationError::InvalidPayload(
                "Element ist nicht sichtbar: about.close".into(),
            ));
        }
        if matches!(
            action.id.as_str(),
            "settings.close"
                | "settings.theme.system"
                | "settings.theme.light"
                | "settings.theme.dark"
                | "settings.data.backup"
                | "settings.data.reset"
        ) && !state.is_settings_open()
        {
            return Err(ApplicationError::InvalidPayload(format!(
                "Element ist nicht sichtbar: {}",
                action.id
            )));
        }
        if action.id == "error.dialog.close" && state.critical_error().is_none() {
            return Err(ApplicationError::InvalidPayload(
                "Element ist nicht sichtbar: error.dialog.close".into(),
            ));
        }

        let enabled = match action.id.as_str() {
            "main.submit" => state.can_submit(),
            "main.cancel" => state.busy,
            "main.retry" => state.can_retry(),
            "main.error.dismiss" => state
                .visible_error()
                .is_some_and(|error| error.severity != application::ErrorSeverity::Critical),
            "error.dialog.close" => state.critical_error().is_some(),
            "main.reset"
            | "main.file-picker"
            | "notes.new"
            | "notes.title"
            | "notes.body"
            | "file.settings"
            | "help.about"
            | "about.close"
            | "settings.close"
            | "settings.data.backup"
            | "settings.data.reset"
            | "settings.theme.system"
            | "settings.theme.light"
            | "settings.theme.dark" => true,
            "notes.save" => state.can_save_note(),
            "notes.archive" | "notes.delete" => state.selected_note_id.is_some(),
            id if id.starts_with("notes.item.") => {
                let index = note_index_from_element_id(id)?;
                index < state.notes.len() && index < 5
            }
            _ => return Ok(()),
        };
        if enabled {
            Ok(())
        } else {
            Err(ApplicationError::InvalidPayload(format!(
                "Element ist deaktiviert: {}",
                action.id
            )))
        }
    }

    pub fn serve(self, bind: &str) -> std::io::Result<()> {
        let listener = TcpListener::bind(bind)?;
        info!(bind, "agent API listening");
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let api = self.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, &api) {
                            tracing::error!(%error, "agent API request failed");
                        }
                    });
                }
                Err(error) => tracing::error!(%error, "agent API accept failed"),
            }
        }
        Ok(())
    }
}

fn main_elements(state: &application::AppState, main_controls_enabled: bool) -> Vec<AgentElement> {
    let mut elements = vec![
        element(
            "main.input",
            main_controls_enabled,
            Some(state.input.clone()),
            "Haupteingabe",
        ),
        element(
            "main.submit",
            main_controls_enabled && state.can_submit(),
            None,
            "Senden",
        ),
        element(
            "main.cancel",
            state.busy && main_controls_enabled,
            None,
            "Abbrechen",
        ),
        element("main.reset", main_controls_enabled, None, "Zurücksetzen"),
        element(
            "main.file-picker",
            main_controls_enabled,
            Some(state.selected_file.clone()),
            "Datei auswählen",
        ),
        element(
            "main.selected-file",
            true,
            Some(state.selected_file.clone()),
            "Ausgewählter Dateipfad",
        ),
        element(
            "main.status",
            true,
            Some(state.error_message.as_deref().map_or_else(
                || state.status.to_string(),
                |error| format!("Fehler: {error}"),
            )),
            "Anwendungsstatus",
        ),
        element(
            "main.tasks",
            true,
            Some(task_summary(state)),
            "Hintergrundvorgänge",
        ),
    ];
    if state
        .visible_error()
        .is_some_and(|error| error.severity != application::ErrorSeverity::Critical)
    {
        elements.push(element(
            "main.error",
            true,
            state.visible_error().map(error_value),
            "Fehlerhinweis",
        ));
        elements.push(element(
            "main.retry",
            main_controls_enabled && state.can_retry(),
            None,
            "Erneut versuchen",
        ));
        elements.push(element(
            "main.error.dismiss",
            main_controls_enabled,
            None,
            "Hinweis schließen",
        ));
    }
    elements
}

fn note_elements(state: &application::AppState, main_controls_enabled: bool) -> Vec<AgentElement> {
    let mut elements = vec![
        element(
            "notes.list",
            true,
            Some(state.notes.len().to_string()),
            "Notizliste",
        ),
        element("notes.new", main_controls_enabled, None, "Neue Notiz"),
        element(
            "notes.title",
            main_controls_enabled,
            Some(state.note_title.clone()),
            "Notiztitel",
        ),
        element(
            "notes.body",
            main_controls_enabled,
            Some(state.note_body.clone()),
            "Notizinhalt",
        ),
        element(
            "notes.save",
            main_controls_enabled && state.can_save_note(),
            None,
            "Speichern",
        ),
        element(
            "notes.archive",
            main_controls_enabled && state.selected_note_id.is_some(),
            None,
            "Archivieren",
        ),
        element(
            "notes.delete",
            main_controls_enabled && state.selected_note_id.is_some(),
            None,
            "Löschen",
        ),
    ];
    let note = state.notes.first();
    elements.push(element(
        "notes.item.0",
        main_controls_enabled && note.is_some(),
        Some(note.map_or_else(String::new, |note| note.id.clone())),
        note.map_or("Leerer Notizplatz", |note| note.title.as_str()),
    ));
    let note = state.notes.get(1);
    elements.push(element(
        "notes.item.1",
        main_controls_enabled && note.is_some(),
        Some(note.map_or_else(String::new, |note| note.id.clone())),
        note.map_or("Leerer Notizplatz", |note| note.title.as_str()),
    ));
    let note = state.notes.get(2);
    elements.push(element(
        "notes.item.2",
        main_controls_enabled && note.is_some(),
        Some(note.map_or_else(String::new, |note| note.id.clone())),
        note.map_or("Leerer Notizplatz", |note| note.title.as_str()),
    ));
    let note = state.notes.get(3);
    elements.push(element(
        "notes.item.3",
        main_controls_enabled && note.is_some(),
        Some(note.map_or_else(String::new, |note| note.id.clone())),
        note.map_or("Leerer Notizplatz", |note| note.title.as_str()),
    ));
    let note = state.notes.get(4);
    elements.push(element(
        "notes.item.4",
        main_controls_enabled && note.is_some(),
        Some(note.map_or_else(String::new, |note| note.id.clone())),
        note.map_or("Leerer Notizplatz", |note| note.title.as_str()),
    ));
    elements
}

fn shell_elements(main_controls_enabled: bool) -> Vec<AgentElement> {
    vec![
        element(
            "file.settings",
            main_controls_enabled,
            None,
            "Einstellungen",
        ),
        element("help.about", main_controls_enabled, None, "Über"),
    ]
}

fn settings_elements(state: &application::AppState) -> Vec<AgentElement> {
    vec![
        element("settings.dialog", true, None, "Einstellungen"),
        element(
            "settings.theme.system",
            true,
            Some((state.theme_mode == ThemeMode::System).to_string()),
            "Systemeinstellung verwenden",
        ),
        element(
            "settings.theme.light",
            true,
            Some((state.theme_mode == ThemeMode::Light).to_string()),
            "Hell",
        ),
        element(
            "settings.theme.dark",
            true,
            Some((state.theme_mode == ThemeMode::Dark).to_string()),
            "Dunkel",
        ),
        element(
            "settings.data.summary",
            true,
            Some(state.data_summary.clone()),
            "Datenstatus",
        ),
        element(
            "settings.data.backup-path",
            true,
            Some(state.last_backup_path.clone().unwrap_or_default()),
            "Letzte Datensicherung",
        ),
        element("settings.data.backup", true, None, "Daten sichern"),
        element(
            "settings.data.reset",
            true,
            None,
            "Lokale Daten zurücksetzen",
        ),
        element("settings.close", true, None, "Schließen"),
    ]
}

fn about_elements() -> Vec<AgentElement> {
    vec![
        element("about.dialog", true, None, "Über Slint Agent Desktop"),
        element("about.close", true, None, "Schließen"),
    ]
}

fn error_dialog_elements(error: &UiError) -> Vec<AgentElement> {
    vec![
        element(
            "error.dialog",
            true,
            Some(error_value(error)),
            "Kritischer Fehler",
        ),
        element("error.dialog.close", true, None, "Schließen"),
    ]
}

fn error_value(error: &UiError) -> String {
    format!(
        "{}|{}|{}|{}",
        error.code, error.severity, error.user_message, error.diagnostic_message
    )
}

fn task_summary(state: &application::AppState) -> String {
    if state.tasks.is_empty() {
        return String::new();
    }
    state
        .tasks
        .iter()
        .rev()
        .take(3)
        .map(|task| {
            task.progress.map_or_else(
                || format!("{}: {}", task.title, task.status),
                |progress| format!("{}: {} ({}%)", task.title, task.status, progress),
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

impl<S> AgentService for AgentApi<S>
where
    S: application::AppRepository
        + application::NoteRepository
        + application::DataMaintenanceRepository
        + Clone
        + Send
        + Sync
        + 'static,
{
    fn get_state(&self) -> Result<AgentStateResponse, ApplicationError> {
        AgentApi::get_state(self)
    }

    fn inspect_ui(&self) -> Result<UiInspectionResponse, ApplicationError> {
        AgentApi::inspect_ui(self)
    }

    fn execute_command(&self, request: AgentCommandRequest) -> Result<String, ApplicationError> {
        AgentApi::execute_command(self, request)
    }

    fn execute_ui_action(&self, action: AgentActionRequest) -> Result<String, ApplicationError> {
        AgentApi::execute_ui_action(self, action)
    }
}

fn required_argument<'a>(
    arguments: &'a serde_json::Value,
    name: &str,
) -> Result<&'a str, ApplicationError> {
    arguments
        .get(name)
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            ApplicationError::InvalidPayload(format!("Zeichenfolgenargument fehlt: {name}"))
        })
}

fn required_action_value(action: &AgentActionRequest) -> Result<&str, ApplicationError> {
    action.value.as_deref().ok_or_else(|| {
        ApplicationError::InvalidPayload(format!("Wert für Element fehlt: {}", action.id))
    })
}

fn note_index_from_element_id(id: &str) -> Result<usize, ApplicationError> {
    let suffix = id.strip_prefix("notes.item.").ok_or_else(|| {
        ApplicationError::InvalidPayload(format!("unbekanntes Notizelement: {id}"))
    })?;
    suffix
        .parse::<usize>()
        .map_err(|_| ApplicationError::InvalidPayload(format!("ungültiges Notizelement: {id}")))
}

fn unsupported_action(action: &AgentActionRequest) -> ApplicationError {
    ApplicationError::InvalidPayload(format!(
        "nicht unterstützte Aktion oder unbekanntes Element: {} {}",
        action.action, action.id
    ))
}

const MAX_HEADER_BYTES: usize = 8 * 1024;
const MAX_BODY_BYTES: usize = 8 * 1024;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

struct HttpRequest {
    method: String,
    path: String,
    body: String,
}

fn handle_connection(stream: TcpStream, api: &impl AgentService) -> std::io::Result<()> {
    stream.set_read_timeout(Some(CONNECTION_TIMEOUT))?;
    stream.set_write_timeout(Some(CONNECTION_TIMEOUT))?;
    let mut reader = BufReader::new(stream);
    let response = match read_request(&mut reader) {
        Ok(request) => dispatch_request(&request, api),
        Err(HttpRequestError::BadRequest(message)) => json_response(
            400,
            &serde_json::json!({"error":{"code":"invalid_request","message":message}}),
        ),
        Err(HttpRequestError::PayloadTooLarge) => json_response(
            413,
            &serde_json::json!({"error":{"code":"payload_too_large","message":"Anfrage ist zu groß"}}),
        ),
        Err(HttpRequestError::Io(error)) => return Err(error),
    };
    reader.get_mut().write_all(response.as_bytes())
}

fn dispatch_request(request: &HttpRequest, api: &impl AgentService) -> String {
    let method = request.method.as_str();
    let path = request.path.as_str();
    let body = request.body.as_str();
    match (method, path) {
        ("GET", "/health") => json_response(200, &serde_json::json!({"status":"ok"})),
        ("GET", "/agent/state") => json_result(api.get_state()),
        ("GET", "/agent/ui") => json_result(api.inspect_ui()),
        ("POST", "/agent/command") => serde_json::from_str::<AgentCommandRequest>(body)
            .map_err(|error| ApplicationError::InvalidPayload(error.to_string()))
            .and_then(|request| api.execute_command(request))
            .map(|result| serde_json::json!({"result": result}))
            .map_or_else(error_response, |body| json_response(200, &body)),
        ("POST", "/agent/ui/action") => serde_json::from_str::<AgentActionRequest>(body)
            .map_err(|error| ApplicationError::InvalidPayload(error.to_string()))
            .and_then(|request| api.execute_ui_action(request))
            .map(|result| serde_json::json!({"result": result}))
            .map_or_else(error_response, |body| json_response(200, &body)),
        _ => json_response(404, &serde_json::json!({"error":"not_found"})),
    }
}

#[derive(Debug)]
enum HttpRequestError {
    BadRequest(&'static str),
    PayloadTooLarge,
    Io(std::io::Error),
}

impl From<std::io::Error> for HttpRequestError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

fn read_request(reader: &mut impl BufRead) -> Result<HttpRequest, HttpRequestError> {
    let mut request_line = String::new();
    let request_line_bytes = reader.read_line(&mut request_line)?;
    if request_line_bytes == 0 {
        return Err(HttpRequestError::BadRequest("Anfrage fehlt"));
    }
    if request_line_bytes > MAX_HEADER_BYTES {
        return Err(HttpRequestError::PayloadTooLarge);
    }

    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(path), Some(_version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(HttpRequestError::BadRequest("Ungültige Anfragezeile"));
    };

    let mut header_bytes = request_line_bytes;
    let mut content_length = 0;
    loop {
        let mut header = String::new();
        let bytes = reader.read_line(&mut header)?;
        if bytes == 0 {
            return Err(HttpRequestError::BadRequest("Unvollständige Anfrage"));
        }
        header_bytes += bytes;
        if header_bytes > MAX_HEADER_BYTES {
            return Err(HttpRequestError::PayloadTooLarge);
        }
        if header == "\r\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("Content-Length") {
                content_length = value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| HttpRequestError::BadRequest("Ungültige Content-Length"))?;
            }
        }
    }
    if content_length > MAX_BODY_BYTES {
        return Err(HttpRequestError::PayloadTooLarge);
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;
    let body = String::from_utf8(body)
        .map_err(|_| HttpRequestError::BadRequest("Anfragetext ist nicht UTF-8"))?;
    Ok(HttpRequest {
        method: method.to_string(),
        path: path.to_string(),
        body,
    })
}

fn json_result<T: Serialize>(result: Result<T, ApplicationError>) -> String {
    result.map_or_else(error_response, |value| json_response(200, &value))
}

fn json_response(status: u16, body: &impl Serialize) -> String {
    let body = serde_json::to_string(body)
        .unwrap_or_else(|_| "{\"error\":\"serialization_failed\"}".into());
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        413 => "Payload Too Large",
        404 => "Not Found",
        _ => "Error",
    };
    format!("HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

fn error_response(error: ApplicationError) -> String {
    let ui_error = UiError::from_application_error(&error);
    json_response(
        400,
        &serde_json::json!({
            "error": {
                "code": ui_error.code.to_string(),
                "message": error.to_string(),
                "user_message": ui_error.user_message,
                "diagnostic_message": ui_error.diagnostic_message,
                "recoverable": ui_error.recoverable,
                "element": null,
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrastructure::MemoryRepository;
    use std::io::Cursor;
    use std::{thread, time::Duration};

    fn api() -> AgentApi<MemoryRepository> {
        AgentApi::new(MemoryRepository::with_default_settings().build_service())
    }

    fn wait_for_submission(api: &AgentApi<MemoryRepository>) -> AgentStateResponse {
        for _ in 0..50 {
            let state = api.get_state().unwrap();
            if !state.busy {
                return state;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("Übermittlung wurde nicht innerhalb von 500 ms abgeschlossen");
    }

    #[test]
    fn semantic_input_controls_submit_and_command_updates_state() {
        let api = api();
        let initial = api.inspect_ui().unwrap();
        assert!(
            !initial
                .elements
                .iter()
                .find(|element| element.id == "main.submit")
                .unwrap()
                .enabled
        );

        api.execute_ui_action(AgentActionRequest {
            action: "set_value".into(),
            id: "main.input".into(),
            value: Some("Test".into()),
        })
        .unwrap();
        assert!(
            api.inspect_ui()
                .unwrap()
                .elements
                .iter()
                .find(|element| element.id == "main.submit")
                .unwrap()
                .enabled
        );

        api.execute_ui_action(AgentActionRequest {
            action: "click".into(),
            id: "main.submit".into(),
            value: None,
        })
        .unwrap();
        assert_eq!(wait_for_submission(&api).status, "erfolgreich");
    }

    #[test]
    fn about_dialog_appears_in_semantic_tree_only_while_open() {
        let api = api();
        let initial = api.inspect_ui().unwrap();
        assert!(initial
            .elements
            .iter()
            .any(|element| element.id == "help.about"));
        assert!(!initial
            .elements
            .iter()
            .any(|element| element.id == "about.dialog"));

        api.execute_ui_action(AgentActionRequest {
            action: "click".into(),
            id: "help.about".into(),
            value: None,
        })
        .unwrap();
        let opened = api.inspect_ui().unwrap();
        assert!(opened
            .elements
            .iter()
            .any(|element| element.id == "about.dialog"));
        assert!(
            opened
                .elements
                .iter()
                .find(|element| element.id == "about.close")
                .unwrap()
                .enabled
        );

        api.execute_ui_action(AgentActionRequest {
            action: "click".into(),
            id: "about.close".into(),
            value: None,
        })
        .unwrap();
        let closed = api.inspect_ui().unwrap();
        assert!(!closed
            .elements
            .iter()
            .any(|element| element.id == "about.dialog"));
    }

    #[test]
    fn reads_valid_get_and_fragmented_post_requests() {
        let mut get = Cursor::new(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n");
        let request = read_request(&mut get).unwrap();
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/health");
        assert!(request.body.is_empty());

        let body = r#"{"id":"main.input","action":"set_value","value":"Test"}"#;
        let post = format!(
            "POST /agent/ui/action HTTP/1.1\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        );
        let mut fragmented = BufReader::with_capacity(1, Cursor::new(post.into_bytes()));
        let request = read_request(&mut fragmented).unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/agent/ui/action");
        assert_eq!(request.body, body);
    }

    #[test]
    fn rejects_malformed_and_oversized_requests() {
        let mut malformed = Cursor::new(b"GET /health\r\n\r\n");
        assert!(matches!(
            read_request(&mut malformed),
            Err(HttpRequestError::BadRequest("Ungültige Anfragezeile"))
        ));

        let mut invalid_length =
            Cursor::new(b"POST /health HTTP/1.1\r\nContent-Length: nope\r\n\r\n");
        assert!(matches!(
            read_request(&mut invalid_length),
            Err(HttpRequestError::BadRequest("Ungültige Content-Length"))
        ));

        let oversized_header = format!(
            "GET / HTTP/1.1\r\nX: {}\r\n\r\n",
            "x".repeat(MAX_HEADER_BYTES)
        );
        let mut oversized_header = Cursor::new(oversized_header);
        assert!(matches!(
            read_request(&mut oversized_header),
            Err(HttpRequestError::PayloadTooLarge)
        ));

        let mut oversized_body = Cursor::new(format!(
            "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        ));
        assert!(matches!(
            read_request(&mut oversized_body),
            Err(HttpRequestError::PayloadTooLarge)
        ));
    }

    #[test]
    fn rejects_non_utf8_request_bodies() {
        let mut request =
            Cursor::new(b"POST / HTTP/1.1\r\nContent-Length: 1\r\n\r\n\xff".as_slice());
        assert!(matches!(
            read_request(&mut request),
            Err(HttpRequestError::BadRequest("Anfragetext ist nicht UTF-8"))
        ));
    }
}
