#[cfg(feature = "agent-api")]
use agent_api::AgentApi;
#[cfg(feature = "agent-api")]
use application::{AppAction, AppState};
#[cfg(feature = "agent-api")]
use infrastructure::initialize_repository;
use slint::ComponentHandle;
#[cfg(feature = "agent-api")]
use slint::SharedString;
#[cfg(feature = "agent-api")]
use std::{sync::Arc, thread};

slint::include_modules!();

#[cfg(feature = "agent-api")]
fn apply_state(ui: &MainWindow, state: AppState) {
    ui.set_status(state.status.to_string().into());
    if ui.get_input_value().as_str() != state.input {
        ui.set_input_value(state.input.into());
    }
    if ui.get_selected_file().as_str() != state.selected_file {
        ui.set_selected_file(state.selected_file.into());
    }
    if ui.get_about_visible() != state.about_open {
        if state.about_open {
            ui.invoke_show_about();
        } else {
            ui.invoke_hide_about();
        }
    }
}

#[cfg_attr(not(feature = "agent-api"), derive(Default))]
pub struct DesktopApp {
    #[cfg(feature = "agent-api")]
    agent_api: Arc<AgentApi<infrastructure::MemoryRepository>>,
}

#[cfg(feature = "agent-api")]
#[allow(clippy::derivable_impls)]
impl Default for DesktopApp {
    fn default() -> Self {
        let repository = initialize_repository();
        Self {
            agent_api: Arc::new(AgentApi::new(repository.build_service())),
        }
    }
}

impl DesktopApp {
    pub fn new() -> Self {
        #[cfg(feature = "agent-api")]
        return Self::default();
        #[cfg(not(feature = "agent-api"))]
        Self {}
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
        ui.set_app_version(env!("CARGO_PKG_VERSION").into());
        #[cfg(feature = "agent-api")]
        let store = app.agent_api.store();
        #[cfg(feature = "agent-api")]
        let updates = store.subscribe().expect("state subscription");
        #[cfg(feature = "agent-api")]
        apply_state(
            &ui,
            store.current_state().expect("initial application state"),
        );
        #[cfg(feature = "agent-api")]
        {
            let weak = ui.as_weak();
            thread::spawn(move || {
                while let Ok(state) = updates.recv() {
                    if weak
                        .upgrade_in_event_loop(move |ui| apply_state(&ui, state))
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }

        #[cfg(feature = "agent-api")]
        ui.on_input_changed({
            let store = Arc::clone(&store);
            move |value| {
                if let Err(error) = store.dispatch(AppAction::SetInput {
                    value: value.to_string(),
                }) {
                    tracing::error!(%error, "failed to synchronize input");
                }
            }
        });

        #[cfg(feature = "agent-api")]
        ui.on_reset({
            let store = Arc::clone(&store);
            move || {
                if let Err(error) = store.dispatch(AppAction::Reset) {
                    tracing::error!(%error, "failed to reset application");
                }
            }
        });

        ui.on_pick_file({
            let weak = ui.as_weak();
            #[cfg(feature = "agent-api")]
            let store = Arc::clone(&store);
            move || {
                #[cfg(feature = "agent-api")]
                {
                    if let Err(error) = store.dispatch(AppAction::SelectDevelopmentFile) {
                        if let Some(ui) = weak.upgrade() {
                            ui.set_status(format!("Fehler: {error}").into());
                        }
                    }
                }
                #[cfg(not(feature = "agent-api"))]
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_selected_file(path.to_string_lossy().into_owned().into());
                    }
                }
            }
        });

        #[cfg(feature = "agent-api")]
        ui.on_about_visibility_changed({
            let store = Arc::clone(&store);
            move |open| {
                let action = if open {
                    AppAction::OpenAbout
                } else {
                    AppAction::CloseAbout
                };
                if let Err(error) = store.dispatch(action) {
                    tracing::error!(%error, "failed to synchronize about dialog");
                }
            }
        });

        #[cfg(feature = "agent-api")]
        ui.on_submit({
            let store = Arc::clone(&store);
            move |value: SharedString| {
                let _ = value;
                if let Err(error) = store.dispatch(AppAction::Submit) {
                    tracing::error!(%error, "failed to submit value");
                }
            }
        });

        ui.run().unwrap();
    }
}
