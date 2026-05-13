//! 9 ツールの `input_schema` JSON 定数（`tool_defs()` から呼ばれる）。
//!
//! 分離理由: `tools.rs` 本体は ToolCall 型 + tool_use_to_call 変換に専念し、
//! schema は宣言的な JSON のかたまりとして 1 ファイルにまとめる。

use serde_json::{json, Value};

use crate::runtime::anthropic::ToolDef;

/// このワーカーが使える全ツール定義を返す。
/// blast radius / cmd_allowlist の強制は interceptor が行うので、ツール提示は常に同じ。
pub fn tool_defs() -> Vec<ToolDef> {
    vec![
        td("record_artifact",
           "成果物を harness に登録する。`harness record-artifact <name> <path> [--tag T]` に相当。",
           schema_record_artifact()),
        td("report_evidence",
           "gate 用 evidence を JSON で記録する。`harness report-evidence <gate> <json>` に相当。",
           schema_report_evidence()),
        td("request_transition",
           "現ノードの出口 gate を全評価し、全 pass なら次ノードへ。失敗なら advance_rejected。",
           schema_request_transition()),
        td("back", "前ノードへ戻る（理由必須）。", schema_back()),
        td("ask", "人間に構造化質問を積む（選択肢付き）。", schema_ask()),
        td("stuck", "詰まったことを申告（理由必須）── 人間にエスカレ。", schema_stuck()),
        td("edit_file",
           "ファイルを書く（blast radius 内のみ許可、interceptor が enforce）。",
           schema_edit_file()),
        td("run_command",
           "ワークディレクトリでコマンドを実行（cmd_allowlist に強制される）。",
           schema_run_command()),
        td("read_file",
           "ファイルを読む（読みは無害なので blast radius 制限なし）。content を返す。",
           schema_read_file()),
    ]
}

fn td(name: &str, desc: &str, schema: Value) -> ToolDef {
    ToolDef {
        name: name.to_string(),
        description: desc.to_string(),
        input_schema: schema,
    }
}

fn schema_record_artifact() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "path": {"type": "string"},
            "tag": {"type": "string"}
        },
        "required": ["name", "path"]
    })
}

fn schema_report_evidence() -> Value {
    json!({
        "type": "object",
        "properties": {
            "gate": {"type": "string"},
            "json": {"type": "object", "description": "evidence の中身（任意キー）"}
        },
        "required": ["gate", "json"]
    })
}

fn schema_request_transition() -> Value {
    json!({"type": "object", "properties": {}})
}

fn schema_back() -> Value {
    json!({
        "type": "object",
        "properties": {"reason": {"type": "string"}},
        "required": ["reason"]
    })
}

fn schema_ask() -> Value {
    json!({
        "type": "object",
        "properties": {
            "question": {"type": "string"},
            "options": {"type": "array", "items": {"type": "string"}},
            "header": {"type": "string"},
            "kind": {"type": "string"},
            "required": {"type": "boolean"}
        },
        "required": ["question"]
    })
}

fn schema_stuck() -> Value {
    json!({
        "type": "object",
        "properties": {"reason": {"type": "string"}},
        "required": ["reason"]
    })
}

fn schema_edit_file() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {"type": "string"},
            "content": {"type": "string"}
        },
        "required": ["path", "content"]
    })
}

fn schema_run_command() -> Value {
    json!({
        "type": "object",
        "properties": {"cmd": {"type": "string"}},
        "required": ["cmd"]
    })
}

fn schema_read_file() -> Value {
    json!({
        "type": "object",
        "properties": {"path": {"type": "string"}},
        "required": ["path"]
    })
}
