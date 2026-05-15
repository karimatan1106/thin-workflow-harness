//! `harness query` 系 7 ツールの input_schema 定数 ── `tool_schemas.rs` から呼ばれる。
//!
//! 分離理由: 9 個の基本 tool 定義に 7 個追加すると 1 ファイル 200 行制約を超える ──
//! query 系は subprocess dispatch なので独立した module にまとめる。
//!
//! ## tool 一覧（subprocess `harness query <sub>` に 1:1）
//!
//! - `query_outline`     ── outline <file>
//! - `query_symbol`      ── symbol <qname>
//! - `query_refs`        ── refs <qname>
//! - `query_callers`     ── callers <qname>
//! - `query_closure`     ── closure <qname> --depth N --direction in|out|both
//! - `query_impacted_by` ── impacted-by <qname> --depth N
//! - `query_tested_by`   ── tested-by <qname> --depth N
//!
//! description には「いつ使うか」を 1 行で書く ── system_prompt を膨らませる代わりに
//! schema 経由で LLM に伝える。outline 以外の 6 tool は `lang` プロパティで
//! Rust/TS/Py/Go を切り替えられる（auto は qname の `::`/`.` と project root から推定）。

use serde_json::{json, Value};

use crate::runtime::anthropic::ToolDef;

/// query 系 7 tool 定義を返す。順序は固定（最後の `query_tested_by` に呼び出し側で
/// cache_control を載せる ── tool_schemas.rs 側の責務）。
pub fn query_tool_defs() -> Vec<ToolDef> {
    vec![
        td("query_outline",
           "指定ファイルの outline（top-level シンボル列挙）を取得する。改修対象ファイルの全体像を最低限の token で掴むときに使う。",
           schema_outline()),
        td("query_symbol",
           "workspace 内のシンボル qname 検索（旧 find-symbol）。「あの関数どこ？」を解決する。`lang=ts|py|go` で他言語 workspace に問い合わせ可、既定 auto。",
           schema_symbol()),
        td("query_refs",
           "指定 symbol への参照箇所一覧。型/関数を rename/署名変更する前の影響範囲評価に使う。`lang=ts|py|go` で他言語 workspace に問い合わせ可、既定 auto。",
           schema_refs()),
        td("query_callers",
           "指定 function の呼び出し元一覧。関数の挙動を変える前に caller の前提を確認する。`lang=ts|py|go` で他言語 workspace に問い合わせ可、既定 auto。",
           schema_refs()),
        td("query_closure",
           "refs/callers の transitive 閉包（depth/direction 指定）。広範な影響を 1 ホップでなく多段で追う。`lang=ts|py|go` で他言語 workspace に問い合わせ可、既定 auto。",
           schema_closure()),
        td("query_impacted_by",
           "closure direction=in の薄いラッパ ── 「この symbol を変えると壊れる範囲は？」を解決する。`lang=ts|py|go` で他言語 workspace に問い合わせ可、既定 auto。",
           schema_depth()),
        td("query_tested_by",
           "指定 symbol をテストしている test 関数一覧。改修前に「どの test が落ちうるか」を引く。`lang=ts|py|go` で他言語 workspace に問い合わせ可、既定 auto。",
           schema_depth()),
    ]
}

fn td(name: &str, desc: &str, schema: Value) -> ToolDef {
    ToolDef {
        name: name.to_string(),
        description: desc.to_string(),
        input_schema: schema,
        cache_control: None,
    }
}

/// `lang` プロパティ JSON 片 ── 6 tool が共通利用する。
/// outline は対象ファイルの拡張子から自明なので持たない。
fn lang_prop() -> Value {
    json!({
        "type": "string",
        "enum": ["auto", "rust", "ts", "py", "go"],
        "default": "auto",
        "description": "対象言語。auto は qname の `::` (Rust) と project root の Cargo.toml / package.json / pyproject.toml / setup.py / requirements.txt / go.mod から推定。`.` 含み qname は TS/Py/Go 曖昧なため root で決まる。"
    })
}

fn schema_outline() -> Value {
    json!({
        "type": "object",
        "properties": {
            "file": {"type": "string", "description": "outline 対象ファイルパス（cwd 相対 or 絶対）"},
            "format": {"type": "string", "enum": ["text", "json"], "description": "出力形式（既定 text）"}
        },
        "required": ["file"]
    })
}

fn schema_symbol() -> Value {
    json!({
        "type": "object",
        "properties": {
            "qname": {"type": "string", "description": "検索クエリ（symbol 名・部分一致）"},
            "kind": {"type": "string", "description": "symbol kind 絞り込み（関数 / 構造体 等）"},
            "root": {"type": "string", "description": "検索 root（既定: cwd）"},
            "format": {"type": "string", "enum": ["text", "json"]},
            "lang": lang_prop()
        },
        "required": ["qname"]
    })
}

fn schema_refs() -> Value {
    json!({
        "type": "object",
        "properties": {
            "qname": {"type": "string", "description": "対象 symbol の qname"},
            "root": {"type": "string"},
            "format": {"type": "string", "enum": ["text", "json"]},
            "lang": lang_prop()
        },
        "required": ["qname"]
    })
}

fn schema_closure() -> Value {
    json!({
        "type": "object",
        "properties": {
            "qname": {"type": "string"},
            "depth": {"type": "integer", "minimum": 1, "description": "閉包の深さ（既定 2）"},
            "direction": {"type": "string", "enum": ["in", "out", "both"], "description": "方向（既定 in）"},
            "root": {"type": "string"},
            "format": {"type": "string", "enum": ["text", "json"]},
            "lang": lang_prop()
        },
        "required": ["qname"]
    })
}

fn schema_depth() -> Value {
    json!({
        "type": "object",
        "properties": {
            "qname": {"type": "string"},
            "depth": {"type": "integer", "minimum": 1, "description": "閉包の深さ（既定 3）"},
            "root": {"type": "string"},
            "format": {"type": "string", "enum": ["text", "json"]},
            "lang": lang_prop()
        },
        "required": ["qname"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_tool_defs_has_seven_entries() {
        let defs = query_tool_defs();
        assert_eq!(defs.len(), 7, "query tool は 7 種");
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        for required in &[
            "query_outline", "query_symbol", "query_refs", "query_callers",
            "query_closure", "query_impacted_by", "query_tested_by",
        ] {
            assert!(names.contains(required), "missing query tool: {required}");
        }
    }

    #[test]
    fn all_query_tools_lack_cache_control() {
        // cache_control は tool_schemas.rs 側で「最後の tool」に乗せる。
        // ここで個別に乗せるとマーカーが分散して cache prefix が崩れる。
        for d in query_tool_defs() {
            assert!(d.cache_control.is_none(), "query tool '{}' に cache_control が漏れている", d.name);
        }
    }

    #[test]
    fn lang_property_present_on_six_tools_only() {
        // outline 以外の 6 tool に lang が乗っていること。outline には乗らない。
        let defs = query_tool_defs();
        for d in &defs {
            let has_lang = d.input_schema
                .get("properties").and_then(|p| p.get("lang")).is_some();
            if d.name == "query_outline" {
                assert!(!has_lang, "query_outline に lang が漏れている");
            } else {
                assert!(has_lang, "query tool '{}' に lang プロパティが無い", d.name);
                let lang = d.input_schema.get("properties").and_then(|p| p.get("lang")).unwrap();
                assert_eq!(lang.get("default").and_then(|v| v.as_str()), Some("auto"));
                let enum_arr = lang.get("enum").and_then(|v| v.as_array()).unwrap();
                let vals: Vec<&str> = enum_arr.iter().filter_map(|v| v.as_str()).collect();
                assert!(
                    vals.contains(&"auto")
                        && vals.contains(&"rust")
                        && vals.contains(&"ts")
                        && vals.contains(&"py")
                        && vals.contains(&"go"),
                    "lang enum 不足: {:?}",
                    vals
                );
            }
        }
    }
}
