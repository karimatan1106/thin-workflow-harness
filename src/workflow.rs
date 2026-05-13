//! workflow.toml のロードと最小 validate。
//!
//! skeleton ではノードはファイル記載順で linear に扱う（fork/join は後フェーズ）。

use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;

use crate::gate::known_gates;
use crate::spec::Spec;
use crate::state::State;

fn empty_toml_value() -> toml::Value {
    toml::Value::Table(toml::Table::new())
}

/// gate spec（`{ gate = "...", args = { ... } }`）。
#[derive(Debug, Clone, Deserialize)]
pub struct GateSpec {
    pub gate: String,
    #[serde(default = "empty_toml_value")]
    pub args: toml::Value,
}

impl GateSpec {
    /// args を `&toml::Table` として見る（テーブルでなければ空扱い）。
    pub fn args_table(&self) -> toml::Table {
        match &self.args {
            toml::Value::Table(t) => t.clone(),
            _ => toml::Table::new(),
        }
    }
}

/// `[meta]` セクション（最小フィールド + optional）。
#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowMeta {
    pub name: String,
    pub entry: String,
    #[serde(default)]
    pub mandatory_gates: Vec<GateSpec>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
}

/// 1 ノード（最小フィールド + optional は未使用で保持）。
#[derive(Debug, Clone, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub exit_gates: Vec<GateSpec>,
    #[serde(default)]
    pub next: Vec<String>,
    #[serde(default)]
    pub serves: Vec<String>,
    #[serde(default)]
    pub can_append: Option<bool>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub on_reject: Option<toml::Value>,
    #[serde(default)]
    pub context: Option<toml::Value>,
}

impl Node {
    /// 表示名（現状 id をそのまま使う ── `name` フィールドは無いので）。
    pub fn display_name(&self) -> &str {
        &self.id
    }
}

/// workflow.toml のパース結果。
#[derive(Debug, Clone, Deserialize)]
pub struct Workflow {
    pub meta: WorkflowMeta,
    #[serde(default)]
    pub node: Vec<Node>,
}

impl Workflow {
    /// ノード列（ファイル記載順）。
    pub fn nodes(&self) -> &[Node] {
        &self.node
    }
}

/// workflow.toml をロードする。
pub fn load_workflow(path: &Path) -> Result<Workflow, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("workflow.toml 読取失敗 {}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("workflow.toml パース失敗 {}: {e}", path.display()))
}

/// 現ノードを返す。done なら None、そうでなければ `phase_index` 番目（記載順）。
pub fn current_node<'a>(wf: &'a Workflow, state: &State) -> Option<&'a Node> {
    if state.phase_index >= wf.node.len() {
        return None;
    }
    wf.node.get(state.phase_index)
}

/// 最小の静的検証。エラーメッセージの Vec を返す（空なら OK）。
pub fn validate(wf: &Workflow, _spec: Option<&Spec>) -> Vec<String> {
    let mut errs = Vec::new();
    let ids: HashSet<&str> = wf.node.iter().map(|n| n.id.as_str()).collect();

    if !ids.contains(wf.meta.entry.as_str()) {
        errs.push(format!("entry '{}' が実在ノード id でない", wf.meta.entry));
    }
    if wf.node.is_empty() {
        errs.push("ノードが 1 つも無い".to_string());
    }

    let known = known_gates();
    for gs in &wf.meta.mandatory_gates {
        if !known.contains(&gs.gate.as_str()) {
            errs.push(format!("mandatory_gates に未知の gate: {}", gs.gate));
        }
    }
    for n in &wf.node {
        for nx in &n.next {
            if !ids.contains(nx.as_str()) {
                errs.push(format!("ノード '{}' の next '{nx}' が実在ノード id でない", n.id));
            }
        }
        for gs in &n.exit_gates {
            if !known.contains(&gs.gate.as_str()) {
                errs.push(format!("ノード '{}' に未知の gate: {}", n.id, gs.gate));
            }
        }
        if n.r#type.as_deref().unwrap_or("task") == "task" && n.skill.is_none() {
            errs.push(format!("task ノード '{}' に skill が無い", n.id));
        }
    }
    errs
}
