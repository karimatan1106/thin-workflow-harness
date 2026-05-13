//! spec.toml のロード（最小スキーマ、深い検証は後フェーズ）。

use std::path::Path;

use serde::Deserialize;

/// `[meta]` セクション。
#[derive(Debug, Clone, Deserialize)]
pub struct SpecMeta {
    pub intent: String,
    pub status: String,
}

/// `[[requirement]]`（F-NNN）。
#[derive(Debug, Clone, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub tests: Vec<String>,
    #[serde(default)]
    pub rationale: Option<String>,
}

/// `[[acceptance]]`（AC-N）。
#[derive(Debug, Clone, Deserialize)]
pub struct Acceptance {
    pub id: String,
    pub requirement: String,
    pub text: String,
    pub test: String,
}

/// `[[invariant]]`（INV-N）。
#[derive(Debug, Clone, Deserialize)]
pub struct Invariant {
    pub id: String,
    pub text: String,
    pub test: String,
}

/// `[[open_question]]`。
#[derive(Debug, Clone, Deserialize)]
pub struct OpenQuestion {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub answer: Option<String>,
}

/// `[approval]`。
#[derive(Debug, Clone, Deserialize)]
pub struct Approval {
    #[serde(default)]
    pub verdict: String,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// spec.toml のパース結果。
#[derive(Debug, Clone, Deserialize)]
pub struct Spec {
    pub meta: SpecMeta,
    #[serde(default)]
    pub requirement: Vec<Requirement>,
    #[serde(default)]
    pub acceptance: Vec<Acceptance>,
    #[serde(default)]
    pub invariant: Vec<Invariant>,
    #[serde(default)]
    pub open_question: Vec<OpenQuestion>,
    #[serde(default)]
    pub approval: Option<Approval>,
}

/// spec.toml をロードする。
pub fn load_spec(path: &Path) -> Result<Spec, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("spec.toml 読取失敗 {}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("spec.toml パース失敗 {}: {e}", path.display()))
}
