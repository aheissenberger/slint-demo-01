#[cfg(feature = "agent-api")]
use agent_api::{AgentApi, AgentCommandRequest};
#[cfg(feature = "agent-api")]
use infrastructure::initialize_repository;
use slint::ComponentHandle;
#[cfg(feature = "agent-api")]
use slint::SharedString;
#[cfg(feature = "agent-api")]
use slint::{Timer, TimerMode};
#[cfg(feature = "agent-api")]
use std::{sync::Arc, thread, time::Duration};

slint::include_modules!();

pub struct DesktopApp {
    #[cfg(feature = "agent-api")]
    agent_api: Arc<AgentApi<infrastructure::MemoryRepository>>,
}

impl Default for DesktopApp {
    fn default() -> Self {
        #[cfg(feature = "agent-api")]
        let repository = initialize_repository();
        Self {
            #[cfg(feature = "agent-api")]
            agent_api: Arc::new(AgentApi::new(repository.build_service())),
        }
    }
}

impl DesktopApp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run() {
        #[cfg(feature = "agent-api")]
        let app = Self::new();
        #[cfg(feature = "agent-api")]
        let api = Arc::clone(&app.agent_api);
        #[cfg(feature = "agent-api")]
        thread::spawn(move || {
            if let Err(error) = (*api).clone().serve("127.0.0.1:8080") {
                tracing::error!(%error, "agent API stopped");
            }
        });
        let ui = MainWindow::new().unwrap();
        #[cfg(feature = "agent-api")]
        let weak = ui.as_weak();
        #[cfg(feature = "agent-api")]
        let agent_api = Arc::clone(&app.agent_api);
        #[cfg(feature = "agent-api")]
        let sync_timer = Timer::default();

        #[cfg(feature = "agent-api")]
        ui.on_input_changed({
            let agent_api = Arc::clone(&agent_api);
            move |value| {
                let _ = agent_api.execute_ui_action(agent_api::AgentActionRequest {
                    action: "set_value".to_string(),
                    id: "main.input".to_string(),
                    value: Some(value.to_string()),
                });
            }
        });

        #[cfg(feature = "agent-api")]
        ui.on_reset({
            let agent_api = Arc::clone(&agent_api);
            move || {
                let _ = agent_api.execute_ui_action(agent_api::AgentActionRequest {
                    action: "click".to_string(),
                    id: "main.reset".to_string(),
                    value: None,
                });
            }
        });

        #[cfg(feature = "agent-api")]
        ui.on_submit(move |value: SharedString| {
            let ui = weak.unwrap();
            let result = agent_api.execute_command(AgentCommandRequest {
                command: "submit".to_string(),
                arguments: serde_json::json!({ "value": value.as_str() }),
            });
            match result {
                Ok(message) => ui.set_status(message.into()),
                Err(error) => ui.set_status(format!("error: {error}").into()),
            }
        });

        #[cfg(feature = "agent-api")]
        {
            let weak = ui.as_weak();
            let agent_api = Arc::clone(&app.agent_api);
            sync_timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                if let Ok(state) = agent_api.get_state() {
                    ui.set_status(state.status.into());
                }
                if let Ok(inspection) = agent_api.inspect_ui() {
                    if let Some(input) = inspection
                        .elements
                        .iter()
                        .find(|element| element.id == "main.input")
                        .and_then(|element| element.value.as_deref())
                    {
                        if ui.get_input_value().as_str() != input {
                            ui.set_input_value(input.into());
                        }
                    }
                }
            });
        }

        ui.run().unwrap();
    }
}
