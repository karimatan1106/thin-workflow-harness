//! スクリプト worker のスクリプト形式（TOML）のパース。
//!
//! ```toml
//! [[step]]
//! node = "node1"
//! actions = [
//!   { create_file = { path = "n1_out.txt", content = "done" } },
//!   { record_artifact = { name = "out1", path = "n1_out.txt" } },
//!   { report_evidence = { gate = "done1", json = "{}" } },
//!   { request_transition = {} },
//! ]
//! ```
//!
//! step は「現ノード id ＝ step.node」でマッチ。同ノードが再 spawn で複数回呼ばれるなら
//! step を複数並べる（runtime が未消費 step を順に消費する）。

use serde::Deserialize;

use crate::runtime::worker::WorkerAction;

/// スクリプト全体（`[[step]]` の列）。
#[derive(Debug, Clone, Deserialize)]
pub struct Script {
    #[serde(default)]
    pub step: Vec<RawStep>,
}

/// 1 step（生パース結果）。
#[derive(Debug, Clone, Deserialize)]
pub struct RawStep {
    pub node: String,
    #[serde(default)]
    pub actions: Vec<RawAction>,
}

/// step を内部表現に変換したもの。
#[derive(Debug, Clone)]
pub struct Step {
    pub node: String,
    pub actions: Vec<WorkerAction>,
}

/// 1 action（TOML の `{ kind = { ...args } }` をタグ無し union として受ける）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RawAction {
    CreateFile { path: String, content: String },
    EditFile { path: String, content: String },
    WriteFile { path: String, content: String },
    RunCommand { cmd: String },
    RecordArtifact { name: String, path: String },
    ReportEvidence { gate: String, #[serde(default = "empty_json")] json: String },
    RequestTransition {},
    Back { reason: String },
    Ask {
        question: String,
        #[serde(default)]
        options: Vec<String>,
        #[serde(default)]
        required: bool,
    },
    Stuck { reason: String },
}

fn empty_json() -> String {
    "{}".to_string()
}

impl From<RawAction> for WorkerAction {
    fn from(r: RawAction) -> Self {
        match r {
            RawAction::CreateFile { path, content } => WorkerAction::CreateFile { path, content },
            RawAction::EditFile { path, content } => WorkerAction::EditFile { path, content },
            RawAction::WriteFile { path, content } => WorkerAction::EditFile { path, content },
            RawAction::RunCommand { cmd } => WorkerAction::RunCommand { cmd },
            RawAction::RecordArtifact { name, path } => WorkerAction::RecordArtifact { name, path },
            RawAction::ReportEvidence { gate, json } => WorkerAction::ReportEvidence { gate, json },
            RawAction::RequestTransition {} => WorkerAction::RequestTransition,
            RawAction::Back { reason } => WorkerAction::Back { reason },
            RawAction::Ask { question, options, required } => {
                WorkerAction::Ask { question, options, required }
            }
            RawAction::Stuck { reason } => WorkerAction::Stuck { reason },
        }
    }
}

/// スクリプト TOML をロードして Step 列に変換する。
pub fn load_script(path: &std::path::Path) -> Result<Vec<Step>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("スクリプト読取失敗 {}: {e}", path.display()))?;
    let parsed: Script =
        toml::from_str(&text).map_err(|e| format!("スクリプトパース失敗 {}: {e}", path.display()))?;
    Ok(parsed
        .step
        .into_iter()
        .map(|rs| Step {
            node: rs.node,
            actions: rs.actions.into_iter().map(WorkerAction::from).collect(),
        })
        .collect())
}
