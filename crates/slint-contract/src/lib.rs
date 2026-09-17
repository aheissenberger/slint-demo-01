use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdLocation {
    pub path: PathBuf,
    pub line: usize,
}

pub fn collect_agent_ids(ui_root: &Path) -> Result<BTreeMap<String, Vec<AgentIdLocation>>, String> {
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

    let mut ids = BTreeMap::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        for (line, id) in scan_agent_ids(&source) {
            ids.entry(id)
                .or_insert_with(Vec::new)
                .push(AgentIdLocation {
                    path: path.clone(),
                    line,
                });
        }
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

fn scan_agent_ids(source: &str) -> Vec<(usize, String)> {
    let mut ids = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let mut cursor = 0;
        while let Some(relative) = line[cursor..].find("agent-id") {
            let start = cursor + relative;
            let after_name = start + "agent-id".len();
            if line[..start].chars().last().is_some_and(|character| {
                character.is_ascii_alphanumeric() || character == '_' || character == '-'
            }) || line[after_name..].chars().next().is_some_and(|character| {
                character.is_ascii_alphanumeric() || character == '_' || character == '-'
            }) {
                cursor = after_name;
                continue;
            }
            let Some(colon) = line[after_name..].find(':') else {
                cursor = after_name;
                continue;
            };
            let value_start = after_name + colon + 1;
            let remainder = line[value_start..].trim_start();
            let Some(remainder) = remainder.strip_prefix('"') else {
                cursor = value_start;
                continue;
            };
            let Some(end_quote) = remainder.find('"') else {
                cursor = value_start;
                continue;
            };
            let id = &remainder[..end_quote];
            if !id.is_empty() {
                ids.push((line_index + 1, id.to_string()));
            }
            cursor = value_start + end_quote + 1;
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::scan_agent_ids;

    #[test]
    fn scans_properties_and_comment_markers_without_matching_longer_names() {
        let source = r#"
            // agent-id: "help.about"
            agent-id: "main.input";
            other-agent-id: "ignored";
        "#;
        assert_eq!(
            scan_agent_ids(source)
                .into_iter()
                .map(|(_, id)| id)
                .collect::<Vec<_>>(),
            vec!["help.about", "main.input"]
        );
    }
}
