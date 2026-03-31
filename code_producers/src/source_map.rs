use serde_json;
use std::fs::File;
use std::io::{BufWriter, Write};

/// Represents a single source map entry mapping a signal operation
/// in a template back to the original .circom source location.
#[derive(Clone, Debug)]
pub struct SourceMapEntry {
    pub template_name: String,
    pub template_id: usize,
    pub signal_name: Option<String>,
    pub statement_type: String,
    pub file_id: usize,
    pub source_file: String,
    pub source_line: usize,
    pub source_column: usize,
}

/// A file referenced in the source map.
#[derive(Clone, Debug)]
pub struct SourceMapFile {
    pub id: usize,
    pub path: String,
}

/// The complete source map for a circom compilation.
#[derive(Clone, Debug)]
pub struct SourceMap {
    pub version: u32,
    pub files: Vec<SourceMapFile>,
    pub mappings: Vec<SourceMapEntry>,
}

impl SourceMap {
    pub fn new() -> Self {
        SourceMap {
            version: 1,
            files: Vec::new(),
            mappings: Vec::new(),
        }
    }

    pub fn add_file(&mut self, id: usize, path: String) {
        // Avoid duplicates
        if !self.files.iter().any(|f| f.id == id) {
            self.files.push(SourceMapFile { id, path });
        }
    }

    pub fn add_entry(&mut self, entry: SourceMapEntry) {
        self.mappings.push(entry);
    }

    pub fn to_json(&self) -> String {
        use serde_json::{json, Map, Value};

        let files: Vec<Value> = self.files.iter().map(|f| {
            json!({
                "id": f.id,
                "path": f.path
            })
        }).collect();

        let mappings: Vec<Value> = self.mappings.iter().map(|m| {
            let mut obj = Map::new();
            obj.insert("templateName".to_string(), json!(m.template_name));
            obj.insert("templateId".to_string(), json!(m.template_id));
            if let Some(ref name) = m.signal_name {
                obj.insert("signalName".to_string(), json!(name));
            }
            obj.insert("statementType".to_string(), json!(m.statement_type));
            obj.insert("fileId".to_string(), json!(m.file_id));
            obj.insert("sourceFile".to_string(), json!(m.source_file));
            obj.insert("sourceLine".to_string(), json!(m.source_line));
            obj.insert("sourceColumn".to_string(), json!(m.source_column));
            Value::Object(obj)
        }).collect();

        let root = json!({
            "version": self.version,
            "files": files,
            "mappings": mappings
        });

        serde_json::to_string_pretty(&root).unwrap()
    }

    pub fn write_to_file(&self, path: &str) -> Result<(), String> {
        let file = File::create(path)
            .map_err(|e| format!("Failed to create source map file {}: {}", path, e))?;
        let mut writer = BufWriter::new(file);
        let json = self.to_json();
        writer
            .write_all(json.as_bytes())
            .map_err(|e| format!("Failed to write source map: {}", e))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush source map: {}", e))?;
        Ok(())
    }
}

use program_structure::ast::*;
use program_structure::file_definition::FileLibrary;

/// Walk a Statement tree and collect source map entries for signal-related
/// operations (substitutions, constraint equalities, declarations of signals,
/// initialization blocks).
pub fn collect_source_map_entries(
    template_name: &str,
    template_id: usize,
    code: &Statement,
    file_library: &FileLibrary,
    source_map: &mut SourceMap,
) {
    walk_statement(template_name, template_id, code, file_library, source_map);
}

fn walk_statement(
    template_name: &str,
    template_id: usize,
    stmt: &Statement,
    file_library: &FileLibrary,
    source_map: &mut SourceMap,
) {
    match stmt {
        Statement::Substitution { meta, var, op, .. } => {
            let stmt_type = match op {
                AssignOp::AssignSignal => "signal_assign",
                AssignOp::AssignConstraintSignal => "constraint_signal_assign",
                AssignOp::AssignVar => "var_assign",
            };
            if let Some(entry) = make_entry(template_name, template_id, Some(var), stmt_type, meta, file_library) {
                source_map.add_entry(entry);
            }
        }
        Statement::MultSubstitution { meta, op, .. } => {
            let stmt_type = match op {
                AssignOp::AssignSignal => "multi_signal_assign",
                AssignOp::AssignConstraintSignal => "multi_constraint_signal_assign",
                AssignOp::AssignVar => "multi_var_assign",
            };
            if let Some(entry) = make_entry(template_name, template_id, None, stmt_type, meta, file_library) {
                source_map.add_entry(entry);
            }
        }
        Statement::UnderscoreSubstitution { meta, op, .. } => {
            let stmt_type = match op {
                AssignOp::AssignSignal => "underscore_signal_assign",
                AssignOp::AssignConstraintSignal => "underscore_constraint_signal_assign",
                AssignOp::AssignVar => "underscore_var_assign",
            };
            if let Some(entry) = make_entry(template_name, template_id, None, stmt_type, meta, file_library) {
                source_map.add_entry(entry);
            }
        }
        Statement::ConstraintEquality { meta, .. } => {
            if let Some(entry) = make_entry(template_name, template_id, None, "constraint_equality", meta, file_library) {
                source_map.add_entry(entry);
            }
        }
        Statement::Declaration { meta, xtype, name, .. } => {
            let stmt_type = match xtype {
                VariableType::Signal(SignalType::Input, _) => "signal_input_declaration",
                VariableType::Signal(SignalType::Output, _) => "signal_output_declaration",
                VariableType::Signal(SignalType::Intermediate, _) => "signal_intermediate_declaration",
                VariableType::Component => "component_declaration",
                VariableType::AnonymousComponent => "anonymous_component_declaration",
                VariableType::Var => "var_declaration",
                VariableType::Bus(_, _, _) => "bus_declaration",
            };
            if let Some(entry) = make_entry(template_name, template_id, Some(name), stmt_type, meta, file_library) {
                source_map.add_entry(entry);
            }
        }
        Statement::InitializationBlock { meta: _, initializations, .. } => {
            for init in initializations {
                walk_statement(template_name, template_id, init, file_library, source_map);
            }
        }
        Statement::Block { stmts, .. } => {
            for s in stmts {
                walk_statement(template_name, template_id, s, file_library, source_map);
            }
        }
        Statement::IfThenElse { if_case, else_case, .. } => {
            walk_statement(template_name, template_id, if_case, file_library, source_map);
            if let Some(else_stmt) = else_case {
                walk_statement(template_name, template_id, else_stmt, file_library, source_map);
            }
        }
        Statement::While { stmt, .. } => {
            walk_statement(template_name, template_id, stmt, file_library, source_map);
        }
        Statement::Return { meta, .. } => {
            if let Some(entry) = make_entry(template_name, template_id, None, "return", meta, file_library) {
                source_map.add_entry(entry);
            }
        }
        Statement::Assert { meta, .. } => {
            if let Some(entry) = make_entry(template_name, template_id, None, "assert", meta, file_library) {
                source_map.add_entry(entry);
            }
        }
        Statement::LogCall { .. } => {
            // Log calls are not signal operations; skip
        }
    }
}

fn make_entry(
    template_name: &str,
    template_id: usize,
    signal_name: Option<&str>,
    statement_type: &str,
    meta: &Meta,
    file_library: &FileLibrary,
) -> Option<SourceMapEntry> {
    let file_id = meta.file_id?;
    let start = meta.get_start();
    let line = file_library.get_line(start, file_id)?;
    let column = file_library.get_column(start, file_id).unwrap_or(1);
    let source_file = file_library
        .get_file_path(file_id)
        .unwrap_or_else(|| "<unknown>".to_string());

    Some(SourceMapEntry {
        template_name: template_name.to_string(),
        template_id,
        signal_name: signal_name.map(|s| s.to_string()),
        statement_type: statement_type.to_string(),
        file_id,
        source_file,
        source_line: line,
        source_column: column,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_map_new_is_empty() {
        let sm = SourceMap::new();
        assert_eq!(sm.version, 1);
        assert!(sm.files.is_empty());
        assert!(sm.mappings.is_empty());
    }

    #[test]
    fn add_file_no_duplicates() {
        let mut sm = SourceMap::new();
        sm.add_file(0, "a.circom".to_string());
        sm.add_file(0, "a.circom".to_string()); // duplicate
        sm.add_file(1, "b.circom".to_string());
        assert_eq!(sm.files.len(), 2);
        assert_eq!(sm.files[0].path, "a.circom");
        assert_eq!(sm.files[1].path, "b.circom");
    }

    #[test]
    fn add_entry_and_count() {
        let mut sm = SourceMap::new();
        sm.add_entry(SourceMapEntry {
            template_name: "Main".to_string(),
            template_id: 0,
            signal_name: Some("out".to_string()),
            statement_type: "signal_assign".to_string(),
            file_id: 0,
            source_file: "test.circom".to_string(),
            source_line: 5,
            source_column: 3,
        });
        sm.add_entry(SourceMapEntry {
            template_name: "Main".to_string(),
            template_id: 0,
            signal_name: None,
            statement_type: "constraint_equality".to_string(),
            file_id: 0,
            source_file: "test.circom".to_string(),
            source_line: 6,
            source_column: 3,
        });
        assert_eq!(sm.mappings.len(), 2);
    }

    #[test]
    fn to_json_round_trip_structure() {
        let mut sm = SourceMap::new();
        sm.add_file(0, "main.circom".to_string());
        sm.add_entry(SourceMapEntry {
            template_name: "Adder".to_string(),
            template_id: 0,
            signal_name: Some("sum".to_string()),
            statement_type: "signal_assign".to_string(),
            file_id: 0,
            source_file: "main.circom".to_string(),
            source_line: 10,
            source_column: 5,
        });

        let json_str = sm.to_json();
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("JSON must be valid");

        assert_eq!(parsed["version"], 1);

        let files = parsed["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["path"], "main.circom");

        let mappings = parsed["mappings"].as_array().unwrap();
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0]["templateName"], "Adder");
        assert_eq!(mappings[0]["signalName"], "sum");
        assert_eq!(mappings[0]["statementType"], "signal_assign");
        assert_eq!(mappings[0]["sourceLine"], 10);
        assert_eq!(mappings[0]["sourceColumn"], 5);
    }

    #[test]
    fn to_json_omits_signal_name_when_none() {
        let mut sm = SourceMap::new();
        sm.add_entry(SourceMapEntry {
            template_name: "T".to_string(),
            template_id: 0,
            signal_name: None,
            statement_type: "constraint_equality".to_string(),
            file_id: 0,
            source_file: "t.circom".to_string(),
            source_line: 1,
            source_column: 1,
        });

        let json_str = sm.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let mapping = &parsed["mappings"][0];
        assert!(
            mapping.get("signalName").is_none(),
            "signalName should be omitted when None"
        );
    }

    #[test]
    fn write_to_file_and_read_back() {
        let mut sm = SourceMap::new();
        sm.add_file(0, "test.circom".to_string());
        sm.add_entry(SourceMapEntry {
            template_name: "Main".to_string(),
            template_id: 0,
            signal_name: Some("x".to_string()),
            statement_type: "var_declaration".to_string(),
            file_id: 0,
            source_file: "test.circom".to_string(),
            source_line: 3,
            source_column: 7,
        });

        let tmpdir = std::env::temp_dir();
        let path = tmpdir.join("circom_srcmap_test.json");
        let path_str = path.to_str().unwrap();

        sm.write_to_file(path_str).expect("write must succeed");

        let content = std::fs::read_to_string(path_str).expect("read must succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&content).expect("JSON must be valid");
        assert_eq!(parsed["version"], 1);
        assert_eq!(parsed["files"][0]["path"], "test.circom");
        assert_eq!(parsed["mappings"][0]["signalName"], "x");

        // Clean up
        std::fs::remove_file(path_str).ok();
    }

    #[test]
    fn multiple_files_in_source_map() {
        let mut sm = SourceMap::new();
        sm.add_file(0, "main.circom".to_string());
        sm.add_file(1, "lib/utils.circom".to_string());
        sm.add_entry(SourceMapEntry {
            template_name: "Main".to_string(),
            template_id: 0,
            signal_name: Some("a".to_string()),
            statement_type: "signal_input_declaration".to_string(),
            file_id: 0,
            source_file: "main.circom".to_string(),
            source_line: 2,
            source_column: 3,
        });
        sm.add_entry(SourceMapEntry {
            template_name: "Utils".to_string(),
            template_id: 1,
            signal_name: Some("b".to_string()),
            statement_type: "signal_output_declaration".to_string(),
            file_id: 1,
            source_file: "lib/utils.circom".to_string(),
            source_line: 5,
            source_column: 3,
        });

        let json_str = sm.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        let files = parsed["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[1]["path"], "lib/utils.circom");

        let mappings = parsed["mappings"].as_array().unwrap();
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0]["fileId"], 0);
        assert_eq!(mappings[1]["fileId"], 1);
    }

    #[test]
    fn all_statement_types_serialized() {
        let types = vec![
            "signal_assign",
            "constraint_signal_assign",
            "var_assign",
            "multi_signal_assign",
            "constraint_equality",
            "signal_input_declaration",
            "signal_output_declaration",
            "signal_intermediate_declaration",
            "component_declaration",
            "var_declaration",
            "return",
            "assert",
        ];

        let mut sm = SourceMap::new();
        for (i, st) in types.iter().enumerate() {
            sm.add_entry(SourceMapEntry {
                template_name: "T".to_string(),
                template_id: 0,
                signal_name: None,
                statement_type: st.to_string(),
                file_id: 0,
                source_file: "t.circom".to_string(),
                source_line: i + 1,
                source_column: 1,
            });
        }

        let json_str = sm.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let mappings = parsed["mappings"].as_array().unwrap();
        assert_eq!(mappings.len(), types.len());
        for (i, st) in types.iter().enumerate() {
            assert_eq!(mappings[i]["statementType"], *st);
        }
    }
}
