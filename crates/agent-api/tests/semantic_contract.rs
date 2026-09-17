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
