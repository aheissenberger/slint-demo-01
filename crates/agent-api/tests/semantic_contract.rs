use agent_api::{ui_component_metadata, AgentActionRequest, AgentApi, AgentCommandRequest};
use infrastructure::MemoryRepository;
use std::{thread, time::Duration};

fn api() -> AgentApi<MemoryRepository> {
    AgentApi::new(MemoryRepository::with_default_settings().build_service())
}

fn wait_for_submission(api: &AgentApi<MemoryRepository>) {
    for _ in 0..50 {
        if !api.get_state().expect("submission state").busy {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("Übermittlung wurde nicht innerhalb von 500 ms abgeschlossen");
}

#[test]
fn component_metadata_defines_the_exposed_capabilities() {
    let input = ui_component_metadata("main.input").expect("input metadata");
    assert_eq!(input.role, "textbox");
    assert_eq!(input.actions, ["get_value", "set_value", "focus"]);

    let submit = ui_component_metadata("main.submit").expect("submit metadata");
    assert_eq!(submit.role, "button");
    assert_eq!(submit.actions, ["click", "focus"]);
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

    api.execute_ui_action(AgentActionRequest {
        action: "set_value".into(),
        id: "main.input".into(),
        value: Some("   ".into()),
    })
    .expect("set whitespace input");
    assert!(
        !api.inspect_ui()
            .expect("inspection after whitespace input")
            .elements
            .iter()
            .find(|element| element.id == "main.submit")
            .expect("submit element after whitespace input")
            .enabled
    );

    api.execute_command(AgentCommandRequest {
        command: "set_input".into(),
        arguments: serde_json::json!({ "value": "Integration test" }),
    })
    .expect("set input command");
    api.execute_command(AgentCommandRequest {
        command: "submit".into(),
        arguments: serde_json::Value::Null,
    })
    .expect("submit command");
    wait_for_submission(&api);
    assert_eq!(api.get_state().expect("state").status, "erfolgreich");
}

#[test]
fn notes_feature_ui_creates_selects_updates_and_archives_notes() {
    let api = api();
    let initial = api.inspect_ui().expect("initial UI inspection");
    assert!(initial
        .elements
        .iter()
        .any(|element| element.id == "notes.new"));
    assert!(
        !initial
            .elements
            .iter()
            .find(|element| element.id == "notes.save")
            .expect("notes.save")
            .enabled
    );

    api.execute_ui_action(AgentActionRequest {
        action: "set_value".into(),
        id: "notes.title".into(),
        value: Some("Erste Notiz".into()),
    })
    .expect("set note title");
    api.execute_ui_action(AgentActionRequest {
        action: "set_value".into(),
        id: "notes.body".into(),
        value: Some("Inhalt".into()),
    })
    .expect("set note body");
    assert!(
        api.inspect_ui()
            .expect("inspection with draft")
            .elements
            .iter()
            .find(|element| element.id == "notes.save")
            .expect("notes.save with draft")
            .enabled
    );

    api.execute_ui_action(AgentActionRequest {
        action: "click".into(),
        id: "notes.save".into(),
        value: None,
    })
    .expect("save note");
    let state = api.get_state().expect("state after save");
    assert_eq!(state.notes.len(), 1);
    assert_eq!(state.note_title, "Erste Notiz");
    assert!(state.selected_note_id.is_some());

    api.execute_ui_action(AgentActionRequest {
        action: "click".into(),
        id: "notes.item.0".into(),
        value: None,
    })
    .expect("select note");
    api.execute_command(AgentCommandRequest {
        command: "set_note_title".into(),
        arguments: serde_json::json!({ "value": "Aktualisierte Notiz" }),
    })
    .expect("set title command");
    api.execute_command(AgentCommandRequest {
        command: "save_note".into(),
        arguments: serde_json::Value::Null,
    })
    .expect("save note command");
    assert_eq!(
        api.get_state().expect("state after update").notes[0].title,
        "Aktualisierte Notiz"
    );

    api.execute_ui_action(AgentActionRequest {
        action: "click".into(),
        id: "notes.archive".into(),
        value: None,
    })
    .expect("archive note");
    let archived = api.get_state().expect("state after archive");
    assert!(archived.notes.is_empty());
    assert!(archived.selected_note_id.is_none());
}

#[test]
fn disabled_submit_cannot_be_activated_through_the_agent_api() {
    let api = api();
    let revision = api.revision().expect("initial revision");

    let error = api
        .execute_ui_action(AgentActionRequest {
            action: "click".into(),
            id: "main.submit".into(),
            value: None,
        })
        .expect_err("disabled submit must reject click actions");

    assert!(error.to_string().contains("Element ist deaktiviert"));
    assert_eq!(
        api.revision().expect("revision after rejected click"),
        revision
    );
    assert_eq!(
        api.get_state().expect("state after rejected click").status,
        "bereit"
    );
}

#[test]
fn missing_required_agent_values_are_rejected_without_mutating_state() {
    let api = api();
    let revision = api.revision().expect("initial revision");

    for request in [
        AgentCommandRequest {
            command: "set_input".into(),
            arguments: serde_json::json!({}),
        },
        AgentCommandRequest {
            command: "select_file".into(),
            arguments: serde_json::json!({}),
        },
        AgentCommandRequest {
            command: "set_theme".into(),
            arguments: serde_json::json!({}),
        },
    ] {
        let error = api
            .execute_command(request)
            .expect_err("missing command argument must be rejected");
        assert!(error.to_string().contains("Zeichenfolgenargument fehlt"));
    }

    let error = api
        .execute_ui_action(AgentActionRequest {
            action: "set_value".into(),
            id: "main.input".into(),
            value: None,
        })
        .expect_err("missing action value must be rejected");
    assert!(error.to_string().contains("Wert für Element fehlt"));
    assert_eq!(api.revision().expect("unchanged revision"), revision);
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
    assert!(
        !opened
            .elements
            .iter()
            .find(|element| element.id == "main.input")
            .expect("main input element")
            .enabled,
        "modal dialog should disable background controls"
    );
    assert!(api
        .execute_ui_action(AgentActionRequest {
            action: "set_value".into(),
            id: "main.input".into(),
            value: Some("blocked".into()),
        })
        .is_err());
    assert!(api
        .execute_ui_action(AgentActionRequest {
            action: "click".into(),
            id: "main.reset".into(),
            value: None,
        })
        .is_err());

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
fn hidden_dialog_controls_cannot_be_activated() {
    let api = api();

    let error = api
        .execute_ui_action(AgentActionRequest {
            action: "click".into(),
            id: "about.close".into(),
            value: None,
        })
        .expect_err("hidden dialog close control must reject activation");

    assert!(error.to_string().contains("nicht sichtbar"));
}

#[test]
fn settings_menu_exposes_and_changes_the_theme_mode() {
    let api = api();
    let initial_state = api.get_state().expect("initial state");
    assert_eq!(initial_state.theme_mode, "system");

    api.execute_ui_action(AgentActionRequest {
        action: "click".into(),
        id: "file.settings".into(),
        value: None,
    })
    .expect("open settings");

    let opened = api
        .inspect_ui()
        .expect("UI inspection after opening settings");
    assert_eq!(opened.screen, "settings");
    assert!(opened
        .elements
        .iter()
        .any(|element| element.id == "settings.dialog"));
    assert_eq!(
        opened
            .elements
            .iter()
            .find(|element| element.id == "settings.theme.system")
            .expect("system theme option")
            .value
            .as_deref(),
        Some("true")
    );
    assert!(opened
        .elements
        .iter()
        .any(|element| element.id == "settings.data.summary"
            && element
                .value
                .as_deref()
                .is_some_and(|value| value.contains("aktive Notizen"))));

    api.execute_ui_action(AgentActionRequest {
        action: "click".into(),
        id: "settings.data.backup".into(),
        value: None,
    })
    .expect("create data backup");
    let backup_state = api.get_state().expect("backup state");
    assert_eq!(
        backup_state.last_backup_path.as_deref(),
        Some("memory://backup")
    );

    api.execute_command(AgentCommandRequest {
        command: "reset_user_data".into(),
        arguments: serde_json::json!({}),
    })
    .expect("reset user data");
    assert!(api.get_state().expect("reset state").notes.is_empty());

    api.execute_ui_action(AgentActionRequest {
        action: "click".into(),
        id: "settings.theme.dark".into(),
        value: None,
    })
    .expect("select dark theme");
    assert_eq!(
        api.get_state().expect("dark theme state").theme_mode,
        "dark"
    );

    let updated = api.inspect_ui().expect("inspection after theme change");
    assert_eq!(
        updated
            .elements
            .iter()
            .find(|element| element.id == "settings.theme.dark")
            .expect("dark theme option")
            .value
            .as_deref(),
        Some("true")
    );

    api.execute_ui_action(AgentActionRequest {
        action: "click".into(),
        id: "settings.close".into(),
        value: None,
    })
    .expect("close settings");
    assert!(!api
        .inspect_ui()
        .expect("inspection after closing settings")
        .elements
        .iter()
        .any(|element| element.id == "settings.dialog"));
}

#[test]
fn hidden_settings_controls_cannot_be_activated() {
    let api = api();

    let error = api
        .execute_ui_action(AgentActionRequest {
            action: "click".into(),
            id: "settings.theme.light".into(),
            value: None,
        })
        .expect_err("hidden theme control must reject activation");

    assert!(error.to_string().contains("nicht sichtbar"));
    assert_eq!(
        api.get_state().expect("unchanged state").theme_mode,
        "system"
    );
}

#[test]
fn agent_can_select_a_file_through_the_same_state_transition_as_the_native_picker() {
    let api = api();
    let initial = api.inspect_ui().expect("initial UI inspection");
    let picker = initial
        .elements
        .iter()
        .find(|element| element.id == "main.file-picker")
        .expect("file picker element");
    assert_eq!(picker.role, "button");
    assert_eq!(picker.value.as_deref(), Some(""));

    api.execute_ui_action(AgentActionRequest {
        action: "set_value".into(),
        id: "main.file-picker".into(),
        value: Some("/tmp/agent-selected.txt".into()),
    })
    .expect("select file");

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

#[test]
fn rejected_ui_actions_are_visible_in_agent_state_and_status() {
    let api = api();

    let error = api
        .execute_ui_action(AgentActionRequest {
            action: "set_value".into(),
            id: "main.file-picker".into(),
            value: Some("\0".into()),
        })
        .unwrap_err();

    let state = api
        .get_state()
        .expect("state after rejected file selection");
    assert_eq!(state.status, "Fehler");
    assert_eq!(
        state.error.as_deref(),
        Some("Die Eingabe konnte nicht verarbeitet werden.")
    );
    assert_eq!(
        state.ui_error.as_ref().map(|error| error.code.to_string()),
        Some("invalid_payload".into())
    );
    assert_eq!(
        state
            .ui_error
            .as_ref()
            .map(|error| error.diagnostic_message.as_str()),
        Some(error.to_string().as_str())
    );

    let status = api
        .inspect_ui()
        .expect("UI inspection after rejected file selection")
        .elements
        .into_iter()
        .find(|element| element.id == "main.status")
        .expect("status element");
    assert_eq!(
        status.value.as_deref(),
        Some("Fehler: Die Eingabe konnte nicht verarbeitet werden.")
    );
}

#[test]
fn command_api_supports_the_same_transitions_as_the_ui_actions() {
    let api = api();

    api.execute_command(AgentCommandRequest {
        command: "set_input".into(),
        arguments: serde_json::json!({ "value": "API command" }),
    })
    .expect("set input via application command");

    let state = api.get_state().expect("state after API command");
    assert_eq!(state.status, "bereit");
    assert!(!state.busy);

    api.execute_command(AgentCommandRequest {
        command: "open_about".into(),
        arguments: serde_json::Value::Null,
    })
    .expect("open about dialog via application command");

    let opened = api
        .inspect_ui()
        .expect("inspection after opening about via command");
    assert!(opened
        .elements
        .iter()
        .any(|element| element.id == "about.dialog"));

    api.execute_command(AgentCommandRequest {
        command: "reset".into(),
        arguments: serde_json::Value::Null,
    })
    .expect("reset via application command");

    let reset = api.get_state().expect("state after reset");
    assert_eq!(reset.status, "bereit");
    assert!(!reset.busy);
}
