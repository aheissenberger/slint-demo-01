#[cfg(feature = "agent-api")]
use agent_api::AgentApi;
use application::{AppAction, AppState};
use infrastructure::initialize_repository;
use slint::ComponentHandle;
use slint::SharedString;
use std::{sync::Arc, thread};

slint::include_modules!();

fn apply_state(ui: &MainWindow, state: AppState) {
    let status = state.error_message.as_deref().map_or_else(
        || state.status.to_string(),
        |error| format!("Fehler: {error}"),
    );
    ui.set_status(status.into());
    ui.set_submit_enabled(state.can_submit());
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
    if state.file_picker_requested {
        ui.invoke_show_native_file_picker();
    }
    if let Some(element_id) = state.focused_element {
        ui.invoke_focus_control(element_id.into());
    }
}

pub struct DesktopApp {
    store: Arc<application::AppStateStore<infrastructure::MemoryRepository>>,
    #[cfg(feature = "agent-api")]
    agent_api: AgentApi<infrastructure::MemoryRepository>,
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

    pub fn run() {
        let app = Self::new();
        let ui = MainWindow::new().unwrap();
        ui.set_app_version(env!("CARGO_PKG_VERSION").into());
        Self::configure_store(&ui, &app.store);
        #[cfg(feature = "agent-api")]
        Self::start_agent_api(&app);
        ui.run().unwrap();
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
        store: &Arc<application::AppStateStore<infrastructure::MemoryRepository>>,
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
        Self::configure_submit(ui, &store);
        Self::configure_focus(ui, &store);
    }

    fn start_state_sync(ui: &MainWindow, updates: std::sync::mpsc::Receiver<AppState>) {
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

    fn configure_input(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::MemoryRepository>>,
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
        store: &Arc<application::AppStateStore<infrastructure::MemoryRepository>>,
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
        store: &Arc<application::AppStateStore<infrastructure::MemoryRepository>>,
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

    fn configure_submit(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::MemoryRepository>>,
    ) {
        let store = Arc::clone(store);
        ui.on_submit(move |value: SharedString| {
            let _ = value;
            if let Err(error) = store.submit() {
                tracing::error!(%error, "failed to submit value");
            }
        });
    }

    fn configure_focus(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::MemoryRepository>>,
    ) {
        let store = Arc::clone(store);
        ui.on_focus_applied(move || {
            if let Err(error) = store.dispatch(AppAction::ClearFocus) {
                tracing::error!(%error, "failed to clear focus request");
            }
        });
    }

    fn configure_file_picker(
        ui: &MainWindow,
        store: &Arc<application::AppStateStore<infrastructure::MemoryRepository>>,
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
        store: &Arc<application::AppStateStore<infrastructure::MemoryRepository>>,
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
}
