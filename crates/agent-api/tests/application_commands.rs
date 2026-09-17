use agent_api::{AgentApi, AgentCommandRequest};
use infrastructure::MemoryRepository;

#[test]
fn unsupported_commands_return_a_stable_validation_error() {
    let api = AgentApi::new(MemoryRepository::with_default_settings().build_service());
    let error = api
        .execute_command(AgentCommandRequest {
            command: "unknown".into(),
            arguments: serde_json::json!({"value": "test"}),
        })
        .expect_err("unknown commands must be rejected");
    assert!(error.to_string().contains("nicht unterstützter Befehl"));
}
