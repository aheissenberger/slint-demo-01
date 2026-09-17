use agent_api::{AgentActionRequest, AgentApi, AgentCommandRequest};
use infrastructure::MemoryRepository;

fn api() -> AgentApi<MemoryRepository> {
    AgentApi::new(MemoryRepository::with_default_settings().build_service())
}

#[test]
fn semantic_contract_exposes_stable_controls_and_state_transitions() {
    let api = api();
    let initial = api.inspect_ui().expect("initial UI inspection");
    let submit = initial
        .elements
        .iter()
        .find(|element| element.id == "main.submit")
        .expect("submit element");
    assert!(!submit.enabled);

    api.execute_ui_action(AgentActionRequest {
        action: "set_value".into(),
        id: "main.input".into(),
        value: Some("Integration test".into()),
    })
    .expect("set input");

    let updated = api.inspect_ui().expect("updated UI inspection");
    assert!(
        updated
            .elements
            .iter()
            .find(|element| element.id == "main.submit")
            .expect("submit element after input")
            .enabled
    );

    api.execute_command(AgentCommandRequest {
        command: "submit".into(),
        arguments: serde_json::json!({ "value": "Integration test" }),
    })
    .expect("submit command");
    assert_eq!(api.get_state().expect("state").status, "success");
}

#[test]
fn about_menu_and_dialog_are_part_of_the_stable_semantic_contract() {
    let api = api();
    let help_about = api
        .inspect_ui()
        .expect("initial UI inspection")
        .elements
        .iter()
        .find(|element| element.id == "help.about")
        .expect("help.about menu item")
        .clone();
    assert_eq!(help_about.role, "menuitem");
    assert!(help_about.enabled);

    api.execute_ui_action(AgentActionRequest {
        action: "click".into(),
        id: "help.about".into(),
        value: None,
    })
    .expect("open about dialog");

    let opened = api.inspect_ui().expect("UI inspection after opening about");
    assert!(
        opened
            .elements
            .iter()
            .any(|element| element.id == "about.dialog"),
        "about dialog should be present once opened"
    );

    api.execute_ui_action(AgentActionRequest {
        action: "click".into(),
        id: "about.close".into(),
        value: None,
    })
    .expect("close about dialog");

    let closed = api.inspect_ui().expect("UI inspection after closing about");
    assert!(
        !closed
            .elements
            .iter()
            .any(|element| element.id == "about.dialog"),
        "about dialog should be removed once closed"
    );
}

#[test]
fn development_file_picker_path_is_configurable_and_selected_on_click() {
    let api = api();
    let initial = api.inspect_ui().expect("initial UI inspection");
    let picker = initial
        .elements
        .iter()
        .find(|element| element.id == "main.file-picker")
        .expect("file picker element");
    assert_eq!(picker.role, "button");
    assert_eq!(picker.value.as_deref(), Some("/workspace/Cargo.toml"));

    api.execute_ui_action(AgentActionRequest {
        action: "set_value".into(),
        id: "main.file-picker".into(),
        value: Some("/tmp/agent-selected.txt".into()),
    })
    .expect("configure development file path");
    api.execute_ui_action(AgentActionRequest {
        action: "click".into(),
        id: "main.file-picker".into(),
        value: None,
    })
    .expect("select configured development file");

    let updated = api.inspect_ui().expect("updated UI inspection");
    let selected_file = updated
        .elements
        .iter()
        .find(|element| element.id == "main.selected-file")
        .expect("selected file element");
    assert_eq!(
        selected_file.value.as_deref(),
        Some("/tmp/agent-selected.txt")
    );
}
