//! 9 ツールの `input_schema` JSON 定数（`tool_defs()` から呼ばれる）。
//!
//! 分離理由: `tools.rs` 本体は ToolCall 型 + tool_use_to_call 変換に専念し、
//! schema は宣言的な JSON のかたまりとして 1 ファイルにまとめる。
//!
//! ## tools 配列の cache_control 戦略
//!
//! 最後のツール定義に `cache_control: ephemeral` を付ける ── Anthropic 仕様で
//! 「最後の cache_control マーカーまでが cache prefix」なので、system blocks(2)
//! ＋ tools 配列 全体が cache 対象になる。これで system + tools の合計が
//! 1024 input token 閾値（cache 作成の最低条件）を確実に超える。

use serde_json::{json, Value};

use crate::runtime::anthropic::{CacheControl, ToolDef};

/// このワーカーが使える全ツール定義を返す。
/// blast radius / cmd_allowlist の強制は interceptor が行うので、ツール提示は常に同じ。
///
/// 最後のツール（`read_file`）には `cache_control: ephemeral` を付け、tools 配列 全体を
/// prompt cache の対象に含める（cache 1024 token 閾値到達のため）。
pub fn tool_defs() -> Vec<ToolDef> {
    let mut defs = vec![
        td("record_artifact",
           "成果物を harness に登録する。`harness record-artifact <name> <path> [--tag T]` に相当。",
           schema_record_artifact()),
        td("report_evidence",
           "gate 用 evidence を JSON で記録する。`harness report-evidence <evidence_key> <json>` に相当。            `gate` 引数には evidence の *key 名* を入れる（例 `human_approval`, `plan_approval`,             `test_result`, `review`, `security_review` 等）── workflow.toml の             `evidence_recorded`/`json_has` gate が参照する key。gate プリミティブの種別名             （`evidence_recorded` 等）を入れるのは誤り。",
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
    ];
    // 最後のツールに cache_control を付ける ── tools 配列全体が cache prefix になる。
    if let Some(last) = defs.last_mut() {
        last.cache_control = Some(CacheControl::ephemeral());
    }
    defs
}

fn td(name: &str, desc: &str, schema: Value) -> ToolDef {
    ToolDef {
        name: name.to_string(),
        description: desc.to_string(),
        input_schema: schema,
        cache_control: None,
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
            "gate": {
                "type": "string",
                "description": "evidence の key 名（例 'human_approval', 'plan_approval',                     'test_result', 'review', 'security_review'）── workflow.toml の                     evidence_recorded/json_has gate が参照する key。                    gate プリミティブの種別名 ('evidence_recorded' 等) を入れないこと。"
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 最後のツールにのみ `cache_control: ephemeral` が乗っている。
    /// これが cache prefix を tools 配列の末尾まで延伸する目印。
    #[test]
    fn last_tool_def_has_cache_control_ephemeral() {
        let defs = tool_defs();
        assert!(!defs.is_empty(), "tool_defs が空");
        for (i, d) in defs.iter().enumerate() {
            let last = i + 1 == defs.len();
            if last {
                let cc = d.cache_control.as_ref().expect("最後のツールに cache_control が無い");
                assert_eq!(cc.kind, "ephemeral");
            } else {
                assert!(d.cache_control.is_none(), "途中のツール {} に cache_control が付いている", d.name);
            }
        }
    }

    /// tools を JSON 化して最後のツールに `cache_control.type == "ephemeral"` が含まれることを確認。
    #[test]
    fn tools_serialize_with_cache_control_on_last() {
        let defs = tool_defs();
        let j = serde_json::to_string(&defs).unwrap();
        // 最後のツール（read_file）に cache_control が付いている。
        assert!(j.contains(r#""name":"read_file""#), "read_file が無い: {j}");
        assert!(j.contains(r#""cache_control":{"type":"ephemeral"}"#), "cache_control 不在: {j}");
    }
}
