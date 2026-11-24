use serde_json::{json, Value};

pub(in crate::providers::agent) fn anthropic_definitions(write: bool) -> Vec<Value> {
    let s = json!({"type":"string"});
    let all = vec![
        json!({
            "name": "list_dir",
            "description": "List up to 300 entries in a relative directory.",
            "input_schema": {"type":"object","properties":{"path":s},"required":["path"]}
        }),
        json!({
            "name": "read_file",
            "description": "Read numbered lines from a text file. At most 400 lines; use ranges for long files.",
            "input_schema": {
                "type":"object",
                "properties":{"path":s,"start_line":{"type":"integer"},"end_line":{"type":"integer"}},
                "required":["path"]
            }
        }),
        json!({
            "name": "read_files",
            "description": "Read several files in ONE call (up to 8): prefer this to one read_file call per file. Each entry has path and optional start_line/end_line (at most 400 lines per file).",
            "input_schema": {
                "type":"object",
                "properties":{"files":{"type":"array","items":{
                    "type":"object",
                    "properties":{"path":s,"start_line":{"type":"integer"},"end_line":{"type":"integer"}},
                    "required":["path"]
                }}},
                "required":["files"]
            }
        }),
        json!({
            "name": "edit_file",
            "description": "Replace one exact unique occurrence.",
            "input_schema": {
                "type":"object",
                "properties":{"path":s,"old_text":s,"new_text":s},
                "required":["path","old_text","new_text"]
            }
        }),
        json!({
            "name": "write_file",
            "description": "Create or overwrite a UTF-8 file.",
            "input_schema": {"type":"object","properties":{"path":s,"content":s},"required":["path","content"]}
        }),
    ];
    if write {
        all
    } else {
        all.into_iter()
            .filter(|tool| matches!(tool["name"].as_str(), Some("list_dir" | "read_file" | "read_files")))
            .collect()
    }
}

pub(in crate::providers::agent) fn openai_definitions(write: bool) -> Vec<Value> {
    anthropic_definitions(write)
        .into_iter()
        .map(super::super::openai_tool)
        .collect()
}
