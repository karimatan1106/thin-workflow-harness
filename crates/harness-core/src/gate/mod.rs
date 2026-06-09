//! gate プリミティブ（20 個）。
//!
//! 各 gate は `(ctx, state) -> (ok, note)` の決定論的関数。未知の名前は `ok=false`。
//! ファイル系は `file_gates`、clean-state は `clean_gates`、state 系は `state_gates`、
//! spec/workflow 系は `spec_gates`。

mod clean_gates;
mod file_gates;
mod glob;
mod spec_gates;
mod state_gates;
mod workflow_diff;
mod workflow_gates;

use std::path::{Path, PathBuf};

use crate::spec::Spec;
use crate::state::State;
use crate::workflow::{Node, Workflow};

pub use glob::{glob_match, glob_paths};

/// 質問キューの 1 エントリ（fold 済み）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub id: String,
    pub kind: String,
    pub question: String,
    pub header: String,
    pub options: Vec<String>,
    pub required: bool,
    pub context_ref: Option<String>,
    pub answered: bool,
    pub answer: Option<String>,
}

/// gate 評価の戻り値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateResult {
    pub ok: bool,
    pub note: String,
}

impl GateResult {
    pub(crate) fn ok(note: impl Into<String>) -> Self {
        GateResult { ok: true, note: note.into() }
    }
    pub(crate) fn fail(note: impl Into<String>) -> Self {
        GateResult { ok: false, note: note.into() }
    }
}

/// gate 評価の文脈。
pub struct GateCtx<'a> {
    pub home: &'a Path,
    pub workflow: Option<&'a Workflow>,
    pub workflow_snapshot: Option<&'a str>,
    pub spec: Option<&'a Spec>,
    pub questions: &'a [Question],
    pub current_node: Option<&'a Node>,
    /// イベント履歴から導出した「到達済みノード id 集合」（`workflow_append_only` の配線追加判定用）。
    pub reached_nodes: &'a [String],
}

impl<'a> GateCtx<'a> {
    /// 最小の ctx（home だけ、他は空）。テスト用。
    pub fn minimal(home: &'a Path) -> Self {
        GateCtx {
            home,
            workflow: None,
            workflow_snapshot: None,
            spec: None,
            questions: &[],
            current_node: None,
            reached_nodes: &[],
        }
    }
}

/// 実装済みの gate プリミティブ名一覧（20 個）。
pub fn known_gates() -> &'static [&'static str] {
    &[
        "file_exists",
        "file_nonempty",
        "max_lines",
        "lines_not_increased",
        "no_regex",
        "cmd_exit_0",
        "git_clean",
        "json_has",
        "json_nonempty",
        "json_in",
        "artifact_registered",
        "evidence_recorded",
        "traceability_closed",
        "workflow_append_only",
        "count_non_decreasing",
        "open_questions_zero",
        "blast_radius_declared",
        "blast_radius_disjoint",
        "spec_refs_exist",
        "no_pending_required_questions",
    ]
}

pub(crate) fn arg_str<'a>(args: &'a toml::Table, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

pub(crate) fn arg_i64(args: &toml::Table, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_integer())
}

pub(crate) fn arg_bool(args: &toml::Table, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

pub(crate) fn resolve(home: &Path, p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() { pb } else { home.join(pb) }
}

/// gate を評価する。
pub fn eval_gate(name: &str, args: &toml::Table, state: &State, ctx: &GateCtx) -> GateResult {
    match name {
        "file_exists" => file_gates::file_exists(args, ctx),
        "file_nonempty" => file_gates::file_nonempty(args, ctx),
        "max_lines" => file_gates::max_lines(args, ctx),
        "lines_not_increased" => file_gates::lines_not_increased(args, state, ctx),
        "no_regex" => file_gates::no_regex(args, ctx),
        "cmd_exit_0" => file_gates::cmd_exit_0(args, ctx),
        "git_clean" => clean_gates::git_clean(args, ctx),
        "json_has" => state_gates::json_has(args, state),
        "json_nonempty" => state_gates::json_nonempty(args, state),
        "json_in" => state_gates::json_in(args, state),
        "artifact_registered" => state_gates::artifact_registered(args, state, ctx),
        "evidence_recorded" => state_gates::evidence_recorded(args, state),
        "count_non_decreasing" => state_gates::count_non_decreasing(args, state),
        "traceability_closed" => spec_gates::traceability_closed(state, ctx),
        "workflow_append_only" => workflow_gates::workflow_append_only(ctx),
        "open_questions_zero" => spec_gates::open_questions_zero(ctx),
        "blast_radius_declared" => spec_gates::blast_radius_declared(ctx),
        "spec_refs_exist" => spec_gates::spec_refs_exist(args, ctx),
        "blast_radius_disjoint" => spec_gates::blast_radius_disjoint(args, ctx),
        "no_pending_required_questions" => spec_gates::no_pending_required_questions(ctx),
        other => GateResult::fail(format!("unknown gate: {other}")),
    }
}
