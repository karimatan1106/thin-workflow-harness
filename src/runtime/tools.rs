//! Anthropic Tool Use にそのまま渡せるツール定義 ＋ `tool_use` の `input` を
//! ハーネス側の `ToolCall` に正規化する変換（`docs/host-capabilities.md` の対応表と整合）。

use serde_json::{json, Value};

use crate::runtime::anthropic::ToolDef;
use crate::runtime::worker::WorkerAction;

/// assistant の tool_use を harness 内で 1 段抽象化した呼び出し ──
/// 大半は既存の `WorkerAction` に流すが、`read_file` だけは戻り値が文字列なので別扱い。
pub enum ToolCall {
    /// 既存 apply 経路（artifact / evidence / transition / back / ask / stuck / edit / run / read 以外）。
    Action(WorkerAction),
    /// `read_file` ── cwd 基準で読んで文字列で返す（blast radius チェックなし、読みは無害）。
    ReadFile(String),
}

/// このワーカーが使える全ツール定義を返す。
/// blast radius / cmd_allowlist の強制は interceptor が行うので、ツール提示は常に同じ。
pub fn tool_defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "record_artifact".into(),
            description: "成果物を harness に登録する。`harness record-artifact <name> <path> [--tag T]` に相当。".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "path": {"type": "string"},
                    "tag": {"type": "string"}
                },
                "required": ["name", "path"]
            }),
        },
        ToolDef {
            name: "report_evidence".into(),
            description: "gate 用 evidence を JSON で記録する。`harness report-evidence <gate> <json>` に相当。".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "gate": {"type": "string"},
                    "json": {"type": "object", "description": "evidence の中身（任意キー）"}
                },
                "required": ["gate", "json"]
            }),
        },
        ToolDef {
            name: "request_transition".into(),
            description: "現ノードの出口 gate を全評価し、全 pass なら次ノードへ。失敗なら advance_rejected。".into(),
            input_schema: json!({"type": "object", "properties": {}}),
        },
        ToolDef {
            name: "back".into(),
            description: "前ノードへ戻る（理由必須）。".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"reason": {"type": "string"}},
                "required": ["reason"]
            }),
        },
        ToolDef {
            name: "ask".into(),
            description: "人間に構造化質問を積む（選択肢付き）。".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "question": {"type": "string"},
                    "options": {"type": "array", "items": {"type": "string"}},
                    "header": {"type": "string"},
                    "kind": {"type": "string"},
                    "required": {"type": "boolean"}
                },
                "required": ["question"]
            }),
        },
        ToolDef {
            name: "stuck".into(),
            description: "詰まったことを申告（理由必須）── 人間にエスカレ。".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"reason": {"type": "string"}},
                "required": ["reason"]
            }),
        },
        ToolDef {
            name: "edit_file".into(),
            description: "ファイルを書く（blast radius 内のみ許可、interceptor が enforce）。".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"]
            }),
        },
        ToolDef {
            name: "run_command".into(),
            description: "ワークディレクトリでコマンドを実行（cmd_allowlist に強制される）。".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"cmd": {"type": "string"}},
                "required": ["cmd"]
            }),
        },
        ToolDef {
            name: "read_file".into(),
            description: "ファイルを読む（読みは無害なので blast radius 制限なし）。content を返す。".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"]
            }),
        },
    ]
}

/// assistant の `tool_use` を `ToolCall` に正規化する。引数不正は `Err`。
pub fn tool_use_to_call(name: &str, input: &Value) -> Result<ToolCall, String> {
    let s = |k: &str| -> Result<String, String> {
        input.get(k).and_then(|v| v.as_str()).map(|v| v.to_string())
            .ok_or_else(|| format!("tool '{name}' に必須キー '{k}'（string）が無い"))
    };
    let s_or = |k: &str, dflt: &str| {
        input.get(k).and_then(|v| v.as_str()).unwrap_or(dflt).to_string()
    };
    let arr = |k: &str| -> Vec<String> {
        input.get(k).and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default()
    };
    let b_or = |k: &str, dflt: bool| {
        input.get(k).and_then(|v| v.as_bool()).unwrap_or(dflt)
    };
    let action = match name {
        "record_artifact" => WorkerAction::RecordArtifact { name: s("name")?, path: s("path")? },
        "report_evidence" => {
            let gate = s("gate")?;
            let json_val = input.get("json").cloned().unwrap_or(Value::Object(Default::default()));
            let json = serde_json::to_string(&json_val).map_err(|e| format!("evidence json 直列化失敗: {e}"))?;
            WorkerAction::ReportEvidence { gate, json }
        }
        "request_transition" => WorkerAction::RequestTransition,
        "back" => WorkerAction::Back { reason: s("reason")? },
        "ask" => WorkerAction::Ask {
            question: s("question")?,
            options: arr("options"),
            required: b_or("required", false),
        },
        "stuck" => WorkerAction::Stuck { reason: s("reason")? },
        "edit_file" => WorkerAction::EditFile { path: s("path")?, content: s_or("content", "") },
        "run_command" => WorkerAction::RunCommand { cmd: s("cmd")? },
        "read_file" => return Ok(ToolCall::ReadFile(s("path")?)),
        other => return Err(format!("未対応ツール: {other}")),
    };
    Ok(ToolCall::Action(action))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_artifact_maps_correctly() {
        match tool_use_to_call("record_artifact", &json!({"name":"a","path":"p"})).unwrap() {
            ToolCall::Action(WorkerAction::RecordArtifact { name, path }) => {
                assert_eq!(name, "a");
                assert_eq!(path, "p");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn report_evidence_serializes_inner_json() {
        match tool_use_to_call("report_evidence", &json!({"gate":"g","json":{"k":1}})).unwrap() {
            ToolCall::Action(WorkerAction::ReportEvidence { gate, json }) => {
                assert_eq!(gate, "g");
                assert!(json.contains("\"k\":1"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn missing_required_key_is_error() {
        assert!(tool_use_to_call("back", &json!({})).is_err());
    }

    #[test]
    fn read_file_maps_to_readfile_variant() {
        match tool_use_to_call("read_file", &json!({"path":"foo.txt"})).unwrap() {
            ToolCall::ReadFile(p) => assert_eq!(p, "foo.txt"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn tool_defs_includes_all_nine() {
        let defs = tool_defs();
        let names: Vec<&str> = defs.iter().map(|t| t.name.as_str()).collect();
        for required in &[
            "record_artifact", "report_evidence", "request_transition", "back", "ask",
            "stuck", "edit_file", "run_command", "read_file",
        ] {
            assert!(names.contains(required), "missing tool: {required}");
        }
    }
}
