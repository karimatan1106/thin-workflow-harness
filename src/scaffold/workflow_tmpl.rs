//! デフォルトワークフロー (research → plan → characterize → implement → test → security
//! → review) を検出結果から組み立てる。`docs/schemas.md` §2.2 準拠。
//!
//! 未検出のコマンドは `false # configure <kind> command ...` にする ── `echo` は exit 0
//! で gate を素通りしてしまうため、未設定なら通さないことを保証する。

use crate::detect::DetectedProject;

pub fn render(d: &DetectedProject) -> String {
    let check = ph(d.check.as_deref().or(d.build.as_deref()), "check");
    let test_cmd = ph(d.test.as_deref(), "test");
    let full = ph(d.full_suite.as_deref().or(d.test.as_deref()), "full-suite");
    let coverage = ph(d.coverage.as_deref(), "coverage");
    let security = if d.gitleaks_available {
        "gitleaks detect --no-git --redact".to_string()
    } else {
        ph_str("security-scan")
    };

    let mut mandatory = format!(
        "  {{ gate = \"cmd_exit_0\", args = {{ cmd = \"{}\" }} }},\n",
        toml_escape(&check)
    );
    if d.gitleaks_available {
        mandatory.push_str("  { gate = \"cmd_exit_0\", args = { cmd = \"gitleaks detect --no-git --redact\" } },\n");
    }

    format!(
        r#"# 自動生成された workflow.toml (`harness init`)
# 検出: lang={lang} build={build} test={test} lint={lint}
# `false # configure ...` プレースホルダは未検出。コマンドを埋めて差し替えてください。
# 詳細は thin-workflow-harness の docs/schemas.md §2.2 (デフォルトワークフローの例) を参照。

[meta]
name = "default-flow"
entry = "research"
# host = "claude-code" でホスト Claude Code、"runtime" は harness ランタイム自身がホスト
host = "claude-code"
mandatory_gates = [
{mandatory}]

[[node]]
id = "research"
skill = "01-research.md"
exit_gates = [
  {{ gate = "open_questions_zero", args = {{}} }},
  {{ gate = "no_pending_required_questions", args = {{}} }},
  {{ gate = "json_has", args = {{ evidence_key = "human_approval", json_path = "verdict", eq = "approved" }} }},
]
next = ["plan"]
on_reject = {{ after = 3, goto = "__human__" }}

[[node]]
id = "plan"
skill = "02-plan.md"
can_append = true
# plan artifact のパスに合わせて max_lines の path を足してください（plan.md 等）。
exit_gates = [
  {{ gate = "artifact_registered", args = {{ name_or_prefix = "plan" }} }},
  {{ gate = "json_has", args = {{ evidence_key = "plan_approval", json_path = "verdict", eq = "approved" }} }},
  {{ gate = "workflow_append_only", args = {{}} }},
]
next = ["characterize"]
on_reject = {{ after = 3, goto = "research" }}

[[node]]
id = "characterize"
skill = "03-characterize.md"
# coverage コマンド未検出時は `false # configure coverage ...` で明示的に fail させる
exit_gates = [
  {{ gate = "cmd_exit_0", args = {{ cmd = "{coverage}" }} }},
]
next = ["implement"]
on_reject = {{ after = 3, goto = "plan" }}

[[node]]
id = "implement"
skill = "04-implement.md"
exit_gates = [
  {{ gate = "artifact_registered", args = {{ name_or_prefix = "impl:" }} }},
  {{ gate = "cmd_exit_0", args = {{ cmd = "{test}" }} }},
]
next = ["test"]
on_reject = {{ after = 3, goto = "plan" }}

[[node]]
id = "test"
skill = "05-test.md"
# baseline (test_count_baseline) は最初の run で確立される ── 無い間は count_non_decreasing は実質緩い
exit_gates = [
  {{ gate = "cmd_exit_0", args = {{ cmd = "{full}" }} }},
  {{ gate = "count_non_decreasing", args = {{ evidence_key = "test_count", baseline_key = "test_count_baseline" }} }},
]
next = ["security"]
on_reject = {{ after = 3, goto = "implement" }}

[[node]]
id = "security"
skill = "06-security.md"
exit_gates = [
  {{ gate = "cmd_exit_0", args = {{ cmd = "{security}" }} }},
  {{ gate = "evidence_recorded", args = {{ key = "security_review" }} }},
]
next = ["review"]
on_reject = {{ after = 3, goto = "implement" }}

[[node]]
id = "review"
skill = "07-review.md"
exit_gates = [
  {{ gate = "traceability_closed", args = {{}} }},
  {{ gate = "json_has", args = {{ evidence_key = "review", json_path = "verdict", eq = "approved" }} }},
]
next = []
on_reject = {{ after = 2, goto = "__human__" }}
"#,
        lang = d.lang.as_deref().unwrap_or("?"),
        build = d.build.as_deref().unwrap_or("?"),
        test = toml_escape(&test_cmd),
        lint = d.lint.as_deref().unwrap_or("?"),
        mandatory = mandatory,
        coverage = toml_escape(&coverage),
        full = toml_escape(&full),
        security = toml_escape(&security),
    )
}

fn ph(cmd: Option<&str>, kind: &str) -> String {
    match cmd {
        Some(c) => c.to_string(),
        None => ph_str(kind),
    }
}

fn ph_str(kind: &str) -> String {
    format!("false # configure {kind} command in .harness/workflow.toml")
}

fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
