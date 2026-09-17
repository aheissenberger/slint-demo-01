use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentDeclarationKind {
    Property,
    CommentMarker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentContractEntry {
    pub id: String,
    pub role: String,
    pub actions: Vec<String>,
    pub path: PathBuf,
    pub line: usize,
    pub kind: AgentDeclarationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentContract {
    pub elements: Vec<AgentContractEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdLocation {
    pub path: PathBuf,
    pub line: usize,
}

pub fn collect_agent_contract(ui_root: &Path) -> Result<AgentContract, String> {
    let mut files = Vec::new();
    collect_slint_files(ui_root, &mut files)?;
    files.sort();

    let entry = ui_root.join("app.slint");
    let compiler = slint_interpreter::Compiler::default();
    let result = spin_on::spin_on(compiler.build_from_path(entry.clone()));
    if result.has_errors() {
        let diagnostics = result
            .diagnostics()
            .map(|diagnostic| format!("{diagnostic:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!(
            "Slint compilation failed for {}:\n{diagnostics}",
            entry.display()
        ));
    }

    let mut elements = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        for mut entry in scan_agent_contract(&source)? {
            entry.path = path.clone();
            elements.push(entry);
        }
    }

    reject_duplicates(&elements)?;

    elements.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(AgentContract { elements })
}

fn reject_duplicates(elements: &[AgentContractEntry]) -> Result<(), String> {
    let mut duplicates = BTreeMap::<&str, Vec<&AgentContractEntry>>::new();
    for entry in elements {
        duplicates.entry(&entry.id).or_default().push(entry);
    }
    let duplicate_messages = duplicates
        .into_iter()
        .filter(|(_, entries)| entries.len() > 1)
        .map(|(id, entries)| {
            let locations = entries
                .iter()
                .map(|entry| format!("{}:{}", entry.path.display(), entry.line))
                .collect::<Vec<_>>()
                .join(", ");
            format!("duplicate agent-id \"{id}\": {locations}")
        })
        .collect::<Vec<_>>();
    if !duplicate_messages.is_empty() {
        return Err(duplicate_messages.join("\n"));
    }
    Ok(())
}

pub fn collect_agent_ids(ui_root: &Path) -> Result<BTreeMap<String, Vec<AgentIdLocation>>, String> {
    let contract = collect_agent_contract(ui_root)?;
    let mut ids = BTreeMap::new();
    for entry in contract.elements {
        ids.entry(entry.id)
            .or_insert_with(Vec::new)
            .push(AgentIdLocation {
                path: entry.path,
                line: entry.line,
            });
    }
    Ok(ids)
}

fn collect_slint_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in
        fs::read_dir(root).map_err(|error| format!("failed to read {}: {error}", root.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to inspect {}: {error}", root.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_slint_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "slint")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn scan_agent_contract(source: &str) -> Result<Vec<AgentContractEntry>, String> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut entries = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let (kind, prefix) = if trimmed.starts_with("// agent-id:") {
            (AgentDeclarationKind::CommentMarker, "// agent-id:")
        } else if trimmed.starts_with("agent-id:") {
            (AgentDeclarationKind::Property, "agent-id:")
        } else {
            continue;
        };
        let Some(id) = quoted_value(trimmed, prefix) else {
            continue;
        };
        if id.is_empty() {
            continue;
        }

        let comment_marker = kind == AgentDeclarationKind::CommentMarker;
        let mut role = None;
        let mut actions = None;
        for candidate in lines.iter().skip(index + 1) {
            let candidate = candidate.trim();
            if let Some(value) = quoted_value(
                candidate,
                if comment_marker {
                    "// agent-role:"
                } else {
                    "agent-role:"
                },
            ) {
                role = Some(value);
                continue;
            }
            if let Some(value) = quoted_value(
                candidate,
                if comment_marker {
                    "// agent-actions:"
                } else {
                    "agent-actions:"
                },
            ) {
                actions = Some(
                    value
                        .split(',')
                        .filter(|action| !action.is_empty())
                        .map(str::to_string)
                        .collect(),
                );
                continue;
            }
            if comment_marker {
                if candidate.is_empty() || candidate.starts_with("//") {
                    continue;
                }
            } else if candidate.is_empty() {
                continue;
            }
            break;
        }

        let role = role
            .ok_or_else(|| format!("agent-id \"{id}\" at line {} has no agent-role", index + 1))?;
        let actions = actions.ok_or_else(|| {
            format!(
                "agent-id \"{id}\" at line {} has no agent-actions",
                index + 1
            )
        })?;
        entries.push(AgentContractEntry {
            id,
            role,
            actions,
            path: PathBuf::new(),
            line: index + 1,
            kind,
        });
    }
    Ok(entries)
}

fn quoted_value(line: &str, prefix: &str) -> Option<String> {
    let value = line.strip_prefix(prefix)?.trim_start().strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::{reject_duplicates, scan_agent_contract, AgentContractEntry, AgentDeclarationKind};
    use std::path::PathBuf;

    #[test]
    fn scans_structured_properties_and_comment_markers() {
        let source = r#"
            // agent-id: "help.about"
            // agent-role: "menuitem"
            // agent-actions: "click"
            agent-id: "main.input";
            agent-role: "textbox";
            agent-actions: "get_value,set_value,focus";
            other-agent-id: "ignored";
        "#;
        let entries = scan_agent_contract(source).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "help.about");
        assert_eq!(entries[0].kind, AgentDeclarationKind::CommentMarker);
        assert_eq!(entries[0].actions, ["click"]);
        assert_eq!(entries[1].id, "main.input");
        assert_eq!(entries[1].role, "textbox");
        assert_eq!(entries[1].actions, ["get_value", "set_value", "focus"]);
    }

    #[test]
    fn rejects_ids_without_metadata() {
        let error = scan_agent_contract(r#"agent-id: "main.input";"#).unwrap_err();
        assert!(error.contains("has no agent-role"));
    }

    #[test]
    fn rejects_duplicate_concrete_ids_with_locations() {
        let entry = |path: &str, line| AgentContractEntry {
            id: "main.submit".into(),
            role: "button".into(),
            actions: vec!["click".into()],
            path: PathBuf::from(path),
            line,
            kind: AgentDeclarationKind::Property,
        };
        let error = reject_duplicates(&[entry("ui/first.slint", 10), entry("ui/second.slint", 20)])
            .unwrap_err();
        assert!(error.contains("duplicate agent-id \"main.submit\""));
        assert!(error.contains("ui/first.slint:10"));
        assert!(error.contains("ui/second.slint:20"));
    }
}
