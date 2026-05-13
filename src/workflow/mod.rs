//! workflow.toml のロードと最小 validate。
//!
//! skeleton ではノードはファイル記載順で linear に扱う（fork/join の並列実行は後フェーズ）。

mod validate;

use std::path::Path;

use serde::Deserialize;

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

/// `{max_tool_calls, max_tokens, max_wall_seconds}`。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Budget {
    #[serde(default)]
    pub max_tool_calls: Option<u64>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub max_wall_seconds: Option<u64>,
}

/// `{after, goto}`。
#[derive(Debug, Clone, Deserialize)]
pub struct OnReject {
    pub after: usize,
    pub goto: String,
}

/// `{include = [...]}`。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Context {
    #[serde(default)]
    pub include: Vec<String>,
}

/// `{tag, gates}`。
#[derive(Debug, Clone, Deserialize)]
pub struct ArtifactTag {
    pub tag: String,
    #[serde(default)]
    pub gates: Vec<GateSpec>,
}

/// `[meta]` セクション。
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
    #[serde(default)]
    pub default_budget: Option<Budget>,
    #[serde(default)]
    pub run_cost_budget: Option<f64>,
    #[serde(default)]
    pub secrets_glob: Vec<String>,
}

/// 1 ノード（全フィールド ── 多くはこのフェーズ未使用で保持）。
#[derive(Debug, Clone, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub serves: Vec<String>,
    #[serde(default)]
    pub exit_gates: Vec<GateSpec>,
    #[serde(default)]
    pub next: Vec<String>,
    #[serde(default)]
    pub branches: Vec<String>,
    #[serde(default)]
    pub wait: Vec<String>,
    #[serde(default)]
    pub on_reject: Option<OnReject>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub can_append: bool,
    #[serde(default)]
    pub context: Option<Context>,
    #[serde(default)]
    pub artifact_tags: Vec<ArtifactTag>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub budget: Option<Budget>,
    #[serde(default)]
    pub cmd_allowlist: Vec<String>,
    #[serde(default)]
    pub network: bool,
    /// ノードに直接付ける blast radius ファイル（任意 ── `serves` 由来とは別に追加できる）。
    #[serde(default)]
    pub files: Vec<String>,
}

impl Node {
    /// 表示名（`name` フィールドは無いので id をそのまま使う）。
    pub fn display_name(&self) -> &str {
        &self.id
    }
    /// ノード種別（既定 "task"）。
    pub fn node_type(&self) -> &str {
        self.r#type.as_deref().unwrap_or("task")
    }

    /// このノードの blast radius ファイル glob 一覧 ──
    /// `serves` の各 F-NNN の `requirement.files` の和集合 ∪ ノード直の `files`。
    pub fn blast_radius(&self, spec: Option<&crate::spec::Spec>) -> Vec<String> {
        let mut out: Vec<String> = self.files.clone();
        if let Some(spec) = spec {
            for fid in &self.serves {
                for r in &spec.requirement {
                    if &r.id == fid {
                        for f in &r.files {
                            if !out.contains(f) {
                                out.push(f.clone());
                            }
                        }
                    }
                }
            }
        }
        out
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
    /// id でノードのインデックスを引く。
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.node.iter().position(|n| n.id == id)
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

pub use validate::validate;
