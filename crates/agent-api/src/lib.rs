use application::{AppService, ApplicationError, ExampleCommand};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
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
    pub enabled: bool,
    pub value: Option<String>,
    pub accessible_label: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeState {
    input: String,
    status: String,
    busy: bool,
}

#[derive(Clone)]
pub struct AgentApi<S> {
    service: AppService<S>,
    state: Arc<Mutex<RuntimeState>>,
}

impl<S> AgentApi<S>
where
    S: application::ExampleRepository + Clone + Send + 'static,
{
    pub fn new(service: AppService<S>) -> Self {
        Self {
            service,
            state: Arc::new(Mutex::new(RuntimeState {
                input: String::new(),
                status: "ready".to_string(),
                busy: false,
            })),
        }
    }

    #[instrument(name = "agent.get_state", skip(self), fields(component = "agent-api"))]
    pub fn get_state(&self) -> Result<AgentStateResponse, ApplicationError> {
        let snapshot = self.service.load_state()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Repository("state lock poisoned".into()))?;
        Ok(AgentStateResponse {
            screen: snapshot.screen,
            status: state.status.clone(),
            busy: state.busy,
        })
    }

    #[instrument(name = "agent.inspect_ui", skip(self), fields(component = "agent-api"))]
    pub fn inspect_ui(&self) -> Result<UiInspectionResponse, ApplicationError> {
        let _ = self.service.load_state()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Repository("state lock poisoned".into()))?;
        Ok(UiInspectionResponse {
            screen: "main".to_string(),
            elements: vec![
                AgentElement {
                    id: "main.input".to_string(),
                    role: "textbox".to_string(),
                    enabled: true,
                    value: Some(state.input.clone()),
                    accessible_label: Some("Main input".to_string()),
                },
                AgentElement {
                    id: "main.submit".to_string(),
                    role: "button".to_string(),
                    enabled: !state.input.trim().is_empty(),
                    value: None,
                    accessible_label: Some("Submit".to_string()),
                },
                AgentElement {
                    id: "main.reset".to_string(),
                    role: "button".to_string(),
                    enabled: true,
                    value: None,
                    accessible_label: Some("Reset".to_string()),
                },
                AgentElement {
                    id: "main.status".to_string(),
                    role: "status".to_string(),
                    enabled: true,
                    value: Some(state.status.clone()),
                    accessible_label: Some("Application status".to_string()),
                },
            ],
        })
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
        let value = request
            .arguments
            .get("value")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let value = self.service.validate_input(value)?;
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| ApplicationError::Repository("state lock poisoned".into()))?;
            state.busy = true;
            state.status = "running".to_string();
        }
        let result = self.service.execute_command(ExampleCommand {
            command: request.command,
            arguments: serde_json::json!({ "value": value }),
        });
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApplicationError::Repository("state lock poisoned".into()))?;
        state.busy = false;
        match &result {
            Ok(_) => state.status = "success".to_string(),
            Err(_) => state.status = "error".to_string(),
        }
        result
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
                let value = self
                    .state
                    .lock()
                    .map_err(|_| ApplicationError::Repository("state lock poisoned".into()))?
                    .input
                    .clone();
                self.execute_command(AgentCommandRequest {
                    command: "submit".into(),
                    arguments: serde_json::json!({ "value": value }),
                })
            }
            "set_value" if action.id == "main.input" => {
                let value = action.value.unwrap_or_default();
                if value.len() > 4096 {
                    return Err(ApplicationError::InvalidPayload(
                        "input exceeds 4096 characters".into(),
                    ));
                }
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| ApplicationError::Repository("state lock poisoned".into()))?;
                state.input = value;
                Ok("value updated".into())
            }
            "click" if action.id == "main.reset" => {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| ApplicationError::Repository("state lock poisoned".into()))?;
                state.input.clear();
                state.status = "ready".into();
                Ok("reset".into())
            }
            "get_value" if action.id == "main.input" => {
                let value = self
                    .state
                    .lock()
                    .map_err(|_| ApplicationError::Repository("state lock poisoned".into()))?
                    .input
                    .clone();
                Ok(value)
            }
            "focus" if action.id == "main.input" || action.id == "main.submit" => {
                Ok(format!("focused {}", action.id))
            }
            _ => Err(ApplicationError::InvalidPayload(format!(
                "unsupported action or element: {} {}",
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
    S: application::ExampleRepository + Clone + Send + 'static,
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
        assert_eq!(api.get_state().unwrap().status, "success");
    }
}
