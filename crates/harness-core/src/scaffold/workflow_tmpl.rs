//! デフォルトワークフロー (research → plan → characterize → implement → test → security
//! → review) を検出結果から組み立てる。`docs/schemas.md` §2.2 準拠。
//!
//! 未検出のコマンドは `false # configure <kind> command ...` にする ── `echo` は exit 0
//! で gate を素通りしてしまうため、未設定なら通さないことを保証する。

use crate::detect::DetectedProject;

pub fn render(d: &DetectedProject) -> String {
    let check = ph(d.check.as_deref().or(d.build.as_deref()), "check");
    let test_cmd = ph(d.test.as_deref(), "test");
    // E2E (L10) は unit と層が違うため自動検出しない ── 必ず明示設定させる。
    let e2e = ph_str("e2e");
    let coverage = ph(d.coverage.as_deref(), "coverage");
    let security = if d.gitleaks_available {
        "gitleaks detect --no-git --redact".to_string()
    } else {
        ph_str("security-scan")
    };
    // @spec 参照を走査する glob (検出言語に応じた拡張子)。未検出は汎用 src 配下。
    let spec_glob = spec_glob_for(d.lang.as_deref());

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
  # master_design_reviewed は「記録の有無」だけでなく中身を強制する:
  # verdict は reviewed/absent/partial のいずれか(勝手な逃げ値を排除)、かつ
  # 何を読んだか(arc42_sections_read)が実体として記録されていること。
  {{ gate = "evidence_recorded", args = {{ key = "master_design_reviewed" }} }},
  {{ gate = "json_in", args = {{ evidence_key = "master_design_reviewed", json_path = "verdict", one_of = "reviewed,absent,partial" }} }},
  {{ gate = "json_nonempty", args = {{ evidence_key = "master_design_reviewed", json_path = "arc42_sections_read" }} }},
  # context_glossary (CONTEXT.md 用語集) の grilling 結果を中身付きで強制 (grill-with-docs 方式。
  # plan モードの代わり)。verdict=created/updated/noop、rationale 必須 (noop でも「なぜ更新不要か」)。
  {{ gate = "evidence_recorded", args = {{ key = "context_glossary" }} }},
  {{ gate = "json_in", args = {{ evidence_key = "context_glossary", json_path = "verdict", one_of = "created,updated,noop" }} }},
  {{ gate = "json_nonempty", args = {{ evidence_key = "context_glossary", json_path = "rationale" }} }},
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
next = ["design-pre"]
on_reject = {{ after = 3, goto = "research" }}

[[node]]
id = "design-pre"
skill = "10-design-pre.md"
# 実装前設計フェーズ ── SDD の核心「設計を先に書く」。plan の後・実装の前に
# マスター設計書を先に更新。設計意図の言語化が要るため Opus 4.8。
model = "claude-opus-4-8"
exit_gates = [
  # 実装前に設計を反映 or 既存設計で足りる旨を中身付きで強制 (noop 逃げ・空 rationale 排除)。
  {{ gate = "evidence_recorded", args = {{ key = "design_pre" }} }},
  {{ gate = "json_in", args = {{ evidence_key = "design_pre", json_path = "verdict", one_of = "updated,noop" }} }},
  {{ gate = "json_nonempty", args = {{ evidence_key = "design_pre", json_path = "rationale" }} }},
  {{ gate = "max_lines", args = {{ path = "docs/architecture/**/*.md", n = 200 }} }},
  {{ gate = "max_lines", args = {{ path = "docs/adr/INDEX.md", n = 200, allow_empty = true }} }},
  {{ gate = "max_lines", args = {{ path = "docs/adr/ADR-*.md", n = 200, allow_empty = true }} }},
]
next = ["characterize"]
on_reject = {{ after = 3, goto = "plan" }}

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
  # 実装中の設計上の気づきを design_note evidence に残す(空配列可)。docdesign が参照し反映。
  {{ gate = "evidence_recorded", args = {{ key = "design_note" }} }},
]
next = ["test"]
on_reject = {{ after = 3, goto = "plan" }}

[[node]]
id = "test"
skill = "05-test.md"
# テストフェーズ ── 機械的作業が中心なので Sonnet 4.6 (default 据え置き)。
model = "claude-sonnet-4-6"
# 回帰 gate (決定論・config 駆動・蓄積): bin/regression_gate.mjs が regression_suites.json の全スイートを
#   実機実行し state/regression_baseline.json と比較。各スイートで pass>=floor-tol かつ fail<=ceiling+tol を
#   満たさなければ exit!=0 → advance 不可 (baseline 比 新規失敗ゼロを毎回強制、既知失敗は baseline に織込み)。
#   harness は gate cmd を cwd=.harness で実行するためパスは `bin/...`。意図的変更時のみ
#   `node bin/regression_gate.mjs --update`(再baseline) / `--ratchet`(floor 引上げ=蓄積)。
# L10: E2E は unit と層が違う別 gate。`{e2e}` を実 E2E コマンドに差し替えること
#   (component 境界欠陥: interface 不一致 / state 伝播 / resource lifecycle / 環境依存。未設定だと fail)。
exit_gates = [
  {{ gate = "cmd_exit_0", args = {{ cmd = "node bin/regression_gate.mjs" }} }},
  {{ gate = "cmd_exit_0", args = {{ cmd = "{e2e}" }} }},
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
# レビューフェーズ ── 最終品質判断の精度が要るため Opus 4.8。コード正しさに専念
# (マスター設計書の作成/修正は次の docdesign node が担う)。
model = "claude-opus-4-8"
exit_gates = [
  {{ gate = "traceability_closed", args = {{}} }},
  {{ gate = "json_has", args = {{ evidence_key = "review", json_path = "verdict", eq = "approved" }} }},
  # L09/L11: 単一 verdict の自己申告でなく、採点 rubric を構造として強制する。
  # review evidence は dimensions (correctness/architecture/test_coverage を最低限
  # スコア付き) を必ず含むこと ── 「動いて見える」を「次元別に評価した」へ外部化する。
  {{ gate = "json_nonempty", args = {{ evidence_key = "review", json_path = "dimensions" }} }},
]
next = ["docdesign"]
on_reject = {{ after = 2, goto = "__human__" }}

[[node]]
id = "docdesign"
skill = "09-docdesign.md"
# 設計書フェーズ ── マスター設計書(architecture/ADR)の作成/修正に専念。設計判断の
# 言語化と整合維持が要るため Opus 4.8。コード正しさ(review)から分離した終端 phase。
model = "claude-opus-4-8"
exit_gates = [
  # master_design_update は「記録の有無」だけでなく中身を強制する:
  # - verdict は updated/noop のいずれか(バグ修正の正当な noop は許すが、no_change 等の逃げ値は排除)
  # - rationale は updated/noop どちらでも必須(なぜ更新したか / なぜ更新不要かを必ず言語化させる)
  {{ gate = "evidence_recorded", args = {{ key = "master_design_update" }} }},
  {{ gate = "json_in", args = {{ evidence_key = "master_design_update", json_path = "verdict", one_of = "updated,noop" }} }},
  {{ gate = "json_nonempty", args = {{ evidence_key = "master_design_update", json_path = "rationale" }} }},
  # ソース中の @spec 参照先が実在するか (sdd.md 規約の実体検証。存在しない仕様書参照を排除)。
  # 言語に合わせて path を調整 (例: src/**/*.ts, crates/**/*.rs)。
  {{ gate = "spec_refs_exist", args = {{ path = "{spec_glob}" }} }},
  # マスター設計書 / ADR の 200 行ルール (AI 駆動開発の token-budget 原則)
  {{ gate = "max_lines", args = {{ path = "docs/architecture/**/*.md", n = 200 }} }},
  {{ gate = "max_lines", args = {{ path = "docs/adr/INDEX.md", n = 200, allow_empty = true }} }},
  {{ gate = "max_lines", args = {{ path = "docs/adr/ADR-*.md", n = 200, allow_empty = true }} }},
  # L12 clean handoff: 終端で working tree に debug/temp/orphan な未追跡残骸が
  # 無いことを保証する (artifacts 除去 次元)。untracked_only=true で正当な実装 diff
  # (追跡済み変更) は許し、未追跡ゴミだけを咎める。ビルド生成物等は ignore に足す。
  {{ gate = "git_clean", args = {{ untracked_only = true, ignore = "target|node_modules|dist|.harness/state" }} }},
]
next = []
on_reject = {{ after = 2, goto = "review" }}
"#,
        lang = d.lang.as_deref().unwrap_or("?"),
        build = d.build.as_deref().unwrap_or("?"),
        test = toml_escape(&test_cmd),
        lint = d.lint.as_deref().unwrap_or("?"),
        mandatory = mandatory,
        coverage = toml_escape(&coverage),
        e2e = toml_escape(&e2e),
        security = toml_escape(&security),
        spec_glob = spec_glob,
    )
}

/// 検出言語から `@spec` 走査用の glob を導く。未検出時は汎用 `src/**/*` (全ソース)。
fn spec_glob_for(lang: Option<&str>) -> &'static str {
    match lang {
        Some("rust") => "**/*.rs",
        Some("typescript") | Some("javascript") | Some("node") => "src/**/*.ts",
        Some("python") => "**/*.py",
        Some("go") => "**/*.go",
        _ => "src/**/*",
    }
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
