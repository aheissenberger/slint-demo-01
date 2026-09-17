use application::{AppCommand, AppService, AppStateStore, ApplicationError};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use tracing::{info, instrument, trace};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStateResponse {
    pub screen: String,
    pub status: String,
    pub busy: bool,
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
        "help.about",
        UiComponentMetadata {
            role: "menuitem",
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
    pub element: Option<String>,
}

#[derive(Clone)]
pub struct AgentApi<S> {
    store: Arc<AppStateStore<S>>,
}

impl<S> AgentApi<S>
where
    S: application::ExampleRepository + Clone + Send + Sync + 'static,
{
    pub fn new(service: AppService<S>) -> Self {
        Self {
            store: Arc::new(AppStateStore::new(service)),
        }
    }

    pub fn store(&self) -> Arc<AppStateStore<S>> {
        Arc::clone(&self.store)
    }

    #[instrument(name = "agent.get_state", skip(self), fields(component = "agent-api"))]
    pub fn get_state(&self) -> Result<AgentStateResponse, ApplicationError> {
        let snapshot = self.store.snapshot()?;
        Ok(AgentStateResponse {
            screen: snapshot.screen,
            status: snapshot.status,
            busy: snapshot.busy,
        })
    }

    #[instrument(name = "agent.inspect_ui", skip(self), fields(component = "agent-api"))]
    pub fn inspect_ui(&self) -> Result<UiInspectionResponse, ApplicationError> {
        let state = self.store.snapshot()?;
        let mut elements = vec![
            element(
                "main.input",
                true,
                Some(state.input.clone()),
                "Haupteingabe",
            ),
            element(
                "main.submit",
                !state.input.trim().is_empty() && !state.busy,
                None,
                "Senden",
            ),
            element("main.reset", true, None, "Zurücksetzen"),
            element(
                "main.file-picker",
                true,
                Some(state.development_file_path.clone()),
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
                Some(state.status.clone()),
                "Anwendungsstatus",
            ),
            element("help.about", true, None, "Über"),
        ];
        if state.about_open {
            elements.push(element(
                "about.dialog",
                true,
                None,
                "Über Slint Agent Desktop",
            ));
            elements.push(element("about.close", true, None, "Schließen"));
        }

        Ok(UiInspectionResponse {
            screen: "main".to_string(),
            elements,
        })
    }

    pub fn revision(&self) -> Result<u64, ApplicationError> {
        Ok(self.store.snapshot()?.revision)
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
            "submit" => AppCommand::Submit,
            "reset" => AppCommand::Reset,
            "open_about" => AppCommand::OpenAbout,
            "close_about" => AppCommand::CloseAbout,
            "select_development_file" => AppCommand::SelectDevelopmentFile,
            "set_input" => {
                let value = request
                    .arguments
                    .get("value")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .unwrap_or_default();
                AppCommand::SetInput(value)
            }
            "set_development_file_path" => {
                let value = request
                    .arguments
                    .get("path")
                    .or_else(|| request.arguments.get("value"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .unwrap_or_default();
                AppCommand::SetDevelopmentFilePath(value)
            }
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
        match action.action.as_str() {
            "click" if action.id == "main.submit" => {
                Ok(self.store.dispatch(AppCommand::Submit)?.message)
            }
            "set_value" if action.id == "main.input" => Ok(self
                .store
                .dispatch(AppCommand::SetInput(action.value.unwrap_or_default()))?
                .message),
            "click" if action.id == "main.reset" => {
                Ok(self.store.dispatch(AppCommand::Reset)?.message)
            }
            "set_value" if action.id == "main.file-picker" => Ok(self
                .store
                .dispatch(AppCommand::SetDevelopmentFilePath(
                    action.value.unwrap_or_default(),
                ))?
                .message),
            "click" if action.id == "main.file-picker" => Ok(self
                .store
                .dispatch(AppCommand::SelectDevelopmentFile)?
                .message),
            "get_value" if action.id == "main.file-picker" => {
                Ok(self.store.snapshot()?.development_file_path)
            }
            "get_value" if action.id == "main.selected-file" => {
                Ok(self.store.snapshot()?.selected_file)
            }
            "click" if action.id == "help.about" => {
                Ok(self.store.dispatch(AppCommand::OpenAbout)?.message)
            }
            "click" if action.id == "about.close" => {
                Ok(self.store.dispatch(AppCommand::CloseAbout)?.message)
            }
            "get_value" if action.id == "main.input" => Ok(self.store.snapshot()?.input),
            "focus"
                if action.id == "main.input"
                    || action.id == "main.submit"
                    || action.id == "main.file-picker" =>
            {
                Ok(format!("{} fokussiert", action.id))
            }
            _ => Err(ApplicationError::InvalidPayload(format!(
                "nicht unterstützte Aktion oder unbekanntes Element: {} {}",
                action.action, action.id
            ))),
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

impl<S> AgentService for AgentApi<S>
where
    S: application::ExampleRepository + Clone + Send + Sync + 'static,
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

fn handle_connection(mut stream: TcpStream, api: &impl AgentService) -> std::io::Result<()> {
    let mut buffer = [0_u8; 16 * 1024];
    let size = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..size]);
    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or_default();
    let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    let response = match (method, path) {
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
    };
    stream.write_all(response.as_bytes())
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
        404 => "Not Found",
        _ => "Error",
    };
    format!("HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

fn error_response(error: ApplicationError) -> String {
    let code = match &error {
        ApplicationError::InvalidPayload(_) => "invalid_input",
        ApplicationError::Domain(_) => "domain_error",
        ApplicationError::Repository(_) => "internal_error",
    };
    json_response(
        400,
        &serde_json::json!({"error":{"code":code,"message":error.to_string()}}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrastructure::MemoryRepository;

    fn api() -> AgentApi<MemoryRepository> {
        AgentApi::new(MemoryRepository::with_default_settings().build_service())
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
        assert_eq!(api.get_state().unwrap().status, "erfolgreich");
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
}
