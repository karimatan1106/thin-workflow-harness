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
# 役割別モデル割り当ての既定値 ── 探索・機械的フェーズはバランス型 Sonnet 4.6。
#   高精度が要る plan/implement/security/review は各ノードで Opus 4.8 に上書きする。
default_model = "claude-sonnet-4-6"
# 既定 budget の現実的下限 ──
#   Haiku でも ApiWorker の system+skill+tools+status で 2000+ input tokens 食うため
#   max_tokens=2000 は即 budget 超過する。実 dogfood (2026-05-13) を踏まえ 8000 を下限に。
#   重い skill / spec を載せるノードは各 [[node]] の `budget` で個別に上書きする。
default_budget = {{ max_tool_calls = 12, max_tokens = 8000, max_wall_seconds = 120 }}
mandatory_gates = [
{mandatory}]

[[node]]
id = "research"
skill = "01-research.md"
# 探索フェーズ ── 広く読むため安価でバランスの取れた Sonnet 4.6 (default 据え置き)。
model = "claude-sonnet-4-6"
exit_gates = [
  {{ gate = "open_questions_zero", args = {{}} }},
  {{ gate = "no_pending_required_questions", args = {{}} }},
  {{ gate = "json_has", args = {{ evidence_key = "human_approval", json_path = "verdict", eq = "approved" }} }},
  {{ gate = "evidence_recorded", args = {{ key = "master_design_reviewed" }} }},
]
next = ["plan"]
on_reject = {{ after = 3, goto = "__human__" }}

[[node]]
id = "plan"
skill = "02-plan.md"
# 計画フェーズ ── 設計判断の質が後続全体を左右するため Opus 4.8。
model = "claude-opus-4-8"
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
# 特性化フェーズ ── 既存挙動を広く把握する探索系なので Sonnet 4.6 (default 据え置き)。
model = "claude-sonnet-4-6"
# coverage コマンド未検出時は `false # configure coverage ...` で明示的に fail させる
exit_gates = [
  {{ gate = "cmd_exit_0", args = {{ cmd = "{coverage}" }} }},
]
next = ["implement"]
on_reject = {{ after = 3, goto = "plan" }}

[[node]]
id = "implement"
skill = "04-implement.md"
# 実装フェーズ ── コード生成の精度が要るため Opus 4.8。
model = "claude-opus-4-8"
exit_gates = [
  {{ gate = "artifact_registered", args = {{ name_or_prefix = "impl:" }} }},
  {{ gate = "cmd_exit_0", args = {{ cmd = "{test}" }} }},
]
next = ["test"]
on_reject = {{ after = 3, goto = "plan" }}

[[node]]
id = "test"
skill = "05-test.md"
# テストフェーズ ── 機械的作業が中心なので Sonnet 4.6 (default 据え置き)。
model = "claude-sonnet-4-6"
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
# セキュリティフェーズ ── 脆弱性検出の見落としを避けるため Opus 4.8。
model = "claude-opus-4-8"
exit_gates = [
  {{ gate = "cmd_exit_0", args = {{ cmd = "{security}" }} }},
  {{ gate = "evidence_recorded", args = {{ key = "security_review" }} }},
]
next = ["review"]
on_reject = {{ after = 3, goto = "implement" }}

[[node]]
id = "review"
skill = "07-review.md"
# レビューフェーズ ── 最終品質判断の精度が要るため Opus 4.8。
model = "claude-opus-4-8"
exit_gates = [
  {{ gate = "traceability_closed", args = {{}} }},
  {{ gate = "json_has", args = {{ evidence_key = "review", json_path = "verdict", eq = "approved" }} }},
  {{ gate = "evidence_recorded", args = {{ key = "master_design_update" }} }},
  # マスター設計書 / ADR の 200 行ルール (AI 駆動開発の token-budget 原則)
  {{ gate = "max_lines", args = {{ path = "docs/architecture/**/*.md", n = 200 }} }},
  {{ gate = "max_lines", args = {{ path = "docs/adr/INDEX.md", n = 200, allow_empty = true }} }},
  {{ gate = "max_lines", args = {{ path = "docs/adr/ADR-*.md", n = 200, allow_empty = true }} }},
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
