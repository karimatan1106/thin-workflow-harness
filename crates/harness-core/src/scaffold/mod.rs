//! `.harness/` レイアウトのスキャフォールド ── workflow.toml / skills / spec.toml /
//! state/.gitkeep / .gitignore を生成する。
//!
//! `docs/onboarding.md` §3 ／ `docs/schemas.md` §2.2「デフォルトワークフローの例」準拠。
//! skill 文面の同梱方法は実装で確定するため、ここではプレースホルダ＋参照案内のみ。

mod docs_tmpl;
mod regression_tmpl;
mod workflow_tmpl;

use std::fs;
use std::path::Path;

use crate::detect::DetectedProject;

/// `.harness/` を target に丸ごと書き出す（既存なら上書き）。
/// 加えて、 `.harness/` の親ディレクトリ (repo root) に `docs/architecture/` と
/// `docs/adr/` の skeleton を生成する (**既存ファイルは skip**、 ユーザー設計書を保護)。
pub fn write_layout(harness_dir: &Path, d: &DetectedProject) -> Result<(), String> {
    fs::create_dir_all(harness_dir).map_err(|e| io_err(harness_dir, e))?;
    let skills = harness_dir.join("skills");
    let state = harness_dir.join("state");
    fs::create_dir_all(&skills).map_err(|e| io_err(&skills, e))?;
    fs::create_dir_all(&state).map_err(|e| io_err(&state, e))?;

    write_file(
        &harness_dir.join("workflow.toml"),
        &workflow_tmpl::render(d),
    )?;
    write_file(&harness_dir.join("spec.toml"), SPEC_TEMPLATE)?;
    write_file(&harness_dir.join(".gitignore"), GITIGNORE)?;
    write_file(&state.join(".gitkeep"), "")?;
    // triage の .out-of-scope: 既却下の要求/アプローチを永続記録し、research が着手前に参照する (死蔵案の再調査回避)。
    let oos = harness_dir.join("out-of-scope");
    fs::create_dir_all(&oos).map_err(|e| io_err(&oos, e))?;
    write_file(&oos.join(".gitkeep"), "")?;
    for (name, body) in SKILL_STUBS {
        write_file(&skills.join(name), body)?;
    }

    // 回帰 gate (言語非依存・config 駆動) を同梱: 検出から suites を生成し、runner を bin/ に置く。
    // test ノードの cmd_exit_0 が `node bin/regression_gate.mjs` でこれを毎回 再実行する。
    let bin = harness_dir.join("bin");
    fs::create_dir_all(&bin).map_err(|e| io_err(&bin, e))?;
    write_file(&bin.join("regression_gate.mjs"), REGRESSION_GATE_MJS)?;
    write_file(&harness_dir.join("regression_suites.json"), &regression_tmpl::render(d))?;
    // 差分 mutation (非ブロッキング品質ゲート・project 非依存): test ノードの mutation_diff
    // evidence を skill が `node bin/mutate-diff.mjs` で生成する。Cargo ワークスペース自動検出。
    write_file(&bin.join("mutate-diff.mjs"), MUTATE_DIFF_MJS)?;
    // 導出元カバレッジ floor ゲート(characterize) と curated バグカタログゲート(test)。project 非依存。
    write_file(&bin.join("characterize_gate.mjs"), CHARACTERIZE_GATE_MJS)?;
    write_file(&bin.join("catalog_gate.mjs"), CATALOG_GATE_MJS)?;
    // OKF v0.1 適合チェック(docs/ 知識バンドル)。docdesign ノードの cmd_exit_0 が
    // `node bin/okf_check.mjs` で実行。fail-safe(docs/ 不在→N/A)・既定 非ブロッキング(OKF_STRICT=1 で強制)。
    write_file(&bin.join("okf_check.mjs"), OKF_CHECK_MJS)?;
    // 差分 mutation ラチェット baseline / equivalent ledger / catalog waiver を seed (版管理する=再浮上/自己採点防止)。
    write_file(&state.join("mutation_baseline.json"), "{}\n")?;
    write_file(&state.join("equivalent_mutants.json"), "[]\n")?;
    write_file(&state.join("catalog_waivers.json"), "[]\n")?;

    // docs/ skeleton (repo root に生成、 既存があれば skip)
    if let Some(repo_root) = harness_dir.parent() {
        match docs_tmpl::write_skeleton(repo_root) {
            Ok(created) if !created.is_empty() => {
                println!(
                    "[OK] docs skeleton 生成 {} 件 (既存はスキップ):",
                    created.len()
                );
                for rel in &created {
                    println!("  + {rel}");
                }
            }
            Ok(_) => {
                println!("[OK] docs skeleton: 全ファイル既存、 スキップ");
            }
            Err(e) => {
                println!("[WARN] docs skeleton 生成失敗: {e}");
            }
        }
        // CONTEXT.md 用語集の形式ガイドを配布 (research/plan の grilling が参照。CONTEXT.md 本体は lazy)。
        let cf = repo_root.join("docs").join("CONTEXT-FORMAT.md");
        if let Some(p) = cf.parent() {
            let _ = fs::create_dir_all(p);
        }
        write_file(&cf, CONTEXT_FORMAT)?;
    }

    Ok(())
}

/// security-only テンプレートを書き出す（`harness init --template security`）。
/// プロジェクト検出に依存しない単一 `security` ノードの workflow + standalone skill。
/// デフォルトワークフローの security フェーズを「それだけ回す」用途。
pub fn write_security_layout(harness_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(harness_dir).map_err(|e| io_err(harness_dir, e))?;
    let skills = harness_dir.join("skills");
    let state = harness_dir.join("state");
    fs::create_dir_all(&skills).map_err(|e| io_err(&skills, e))?;
    fs::create_dir_all(&state).map_err(|e| io_err(&state, e))?;

    write_file(&harness_dir.join("workflow.toml"), SECURITY_WORKFLOW)?;
    write_file(&skills.join("security.md"), SECURITY_SKILL)?;
    write_file(&harness_dir.join("spec.toml"), SPEC_TEMPLATE)?;
    write_file(&harness_dir.join(".gitignore"), GITIGNORE)?;
    write_file(&state.join(".gitkeep"), "")?;
    Ok(())
}

/// preservation(挙動保存=rehost/migration)テンプレートを書き出す（`harness init --template preservation`）。
/// oracle=旧システムの実挙動。research→capture→differential→reconcile→coverage の5ノード + 6 tool + 等価/入力空間テンプレ。
/// COBOL OS-only rehost 等の挙動保存移行用。全ゲート fail-safe・新 gate primitive ゼロ。
pub fn write_preservation_layout(harness_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(harness_dir).map_err(|e| io_err(harness_dir, e))?;
    let skills = harness_dir.join("skills");
    let state = harness_dir.join("state");
    let bin = harness_dir.join("bin");
    let golden = harness_dir.join("golden");
    let preservation = harness_dir.join("preservation");
    for d in [&skills, &state, &bin, &golden, &preservation] {
        fs::create_dir_all(d).map_err(|e| io_err(d, e))?;
    }

    write_file(&harness_dir.join("workflow.toml"), PRESERVATION_WORKFLOW)?;
    write_file(&harness_dir.join("spec.toml"), SPEC_TEMPLATE)?;
    write_file(&harness_dir.join(".gitignore"), PRESERVATION_GITIGNORE)?;
    write_file(&state.join(".gitkeep"), "")?;

    // 5 skill (oracle=旧実挙動。forward の skill は oracle 衝突のため流用しない)。
    write_file(&skills.join("p01-research.md"), P01_RESEARCH)?;
    write_file(&skills.join("p02-capture.md"), P02_CAPTURE)?;
    write_file(&skills.join("p03-differential.md"), P03_DIFFERENTIAL)?;
    write_file(&skills.join("p04-reconcile.md"), P04_RECONCILE)?;
    write_file(&skills.join("p05-coverage.md"), P05_COVERAGE)?;

    // 6 tool (全 fail-safe・project 非依存)。
    write_file(&bin.join("preservation_lib.mjs"), PRESERVATION_LIB_MJS)?;
    write_file(&bin.join("capture_oracle.mjs"), CAPTURE_ORACLE_MJS)?;
    write_file(&bin.join("differential_gate.mjs"), DIFFERENTIAL_GATE_MJS)?;
    write_file(&bin.join("reconcile_gate.mjs"), RECONCILE_GATE_MJS)?;
    write_file(&bin.join("coverage_gate.mjs"), COVERAGE_GATE_MJS)?;
    write_file(&bin.join("db-assert.mjs"), DB_ASSERT_MJS)?;

    // 等価方針 / 入力空間 テンプレ。
    write_file(&harness_dir.join("equivalence.json"), EQUIVALENCE_JSON)?;
    write_file(&preservation.join("input_space.json"), INPUT_SPACE_JSON)?;

    // golden(旧の録画)— 既定空。golden/ 自体は .gitignore 対象だが manifest は構造を残す。
    write_file(&golden.join(".gitkeep"), "")?;
    write_file(&golden.join("manifest.json"), "{\"entries\":[]}\n")?;

    // state seed(ledger/baseline は版管理する=再浮上/自己採点防止。.gitignore で negate)。
    write_file(&state.join("reconcile_ledger.json"), "[]\n")?;
    write_file(&state.join("coverage_ledger.json"), "[]\n")?;
    write_file(&state.join("coverage_baseline.json"), "{}\n")?;
    write_file(&state.join("capture_provenance.json"), "[]\n")?;
    write_file(&state.join("nondeterminism_ledger.json"), "[]\n")?;
    write_file(&state.join("absorbed_divergences.json"), "{}\n")?;
    Ok(())
}

/// security-only テンプレート（バイナリ同梱 ── 旧 harness-plugin-security の中身）。
const SECURITY_WORKFLOW: &str = include_str!("templates/security_workflow.toml");
const SECURITY_SKILL: &str = include_str!("templates/security_skill.md");

/// 回帰 gate ツール（言語非依存・config 駆動。`tool_templates/regression_gate.mjs` を同梱）。
const REGRESSION_GATE_MJS: &str = include_str!("tool_templates/regression_gate.mjs");

/// 差分 mutation ツール（project 非依存。`tool_templates/mutate-diff.mjs` を同梱）。
/// test ノードの mutation_diff evidence を生成。Rust=cargo-mutants --in-diff(変更行のみ)。
const MUTATE_DIFF_MJS: &str = include_str!("tool_templates/mutate-diff.mjs");

/// characterize の導出元カバレッジ floor ゲート（project 非依存・spec.toml の AC/INV 束縛を確認）。
const CHARACTERIZE_GATE_MJS: &str = include_str!("tool_templates/characterize_gate.mjs");
/// curated バグカタログゲート（project 非依存・規則 JSON 無→N/A）。
const CATALOG_GATE_MJS: &str = include_str!("tool_templates/catalog_gate.mjs");
/// OKF v0.1 適合チェッカ（project 非依存・docs/ バンドルの frontmatter+type を検査・fail-safe）。
const OKF_CHECK_MJS: &str = include_str!("tool_templates/okf_check.mjs");

/// preservation(挙動保存)トラック同梱物。oracle=旧実挙動・全 fail-safe・project 非依存。
const PRESERVATION_WORKFLOW: &str = include_str!("templates/preservation_workflow.toml");
const EQUIVALENCE_JSON: &str = include_str!("templates/equivalence.json");
const INPUT_SPACE_JSON: &str = include_str!("templates/input_space.json");
const P01_RESEARCH: &str = include_str!("skill_templates/p01-research.md");
const P02_CAPTURE: &str = include_str!("skill_templates/p02-capture.md");
const P03_DIFFERENTIAL: &str = include_str!("skill_templates/p03-differential.md");
const P04_RECONCILE: &str = include_str!("skill_templates/p04-reconcile.md");
const P05_COVERAGE: &str = include_str!("skill_templates/p05-coverage.md");
const PRESERVATION_LIB_MJS: &str = include_str!("tool_templates/preservation_lib.mjs");
const CAPTURE_ORACLE_MJS: &str = include_str!("tool_templates/capture_oracle.mjs");
const DIFFERENTIAL_GATE_MJS: &str = include_str!("tool_templates/differential_gate.mjs");
const RECONCILE_GATE_MJS: &str = include_str!("tool_templates/reconcile_gate.mjs");
const COVERAGE_GATE_MJS: &str = include_str!("tool_templates/coverage_gate.mjs");
const DB_ASSERT_MJS: &str = include_str!("tool_templates/db-assert.mjs");
/// preservation 専用 .gitignore: golden(録画・大きい/PII)と divergences は ignore、ledger/baseline は版管理(negate)。
const PRESERVATION_GITIGNORE: &str = "state/*.jsonl\nstate/*.lock\ntranscripts/\ngolden/\n!golden/.gitkeep\nstate/divergences.json\nstate/absorbed_divergences.json\n!state/.gitkeep\n!state/reconcile_ledger.json\n!state/coverage_ledger.json\n!state/coverage_baseline.json\n!state/capture_provenance.json\n!state/nondeterminism_ledger.json\n";

/// CONTEXT.md(ドメイン用語集)形式ガイド。research/plan の grilling が `docs/CONTEXT-FORMAT.md` を参照する。
const CONTEXT_FORMAT: &str = include_str!("templates/CONTEXT-FORMAT.md");

fn io_err(p: &Path, e: std::io::Error) -> String {
    format!("{} 操作失敗: {e}", p.display())
}

fn write_file(p: &Path, body: &str) -> Result<(), String> {
    fs::write(p, body).map_err(|e| io_err(p, e))
}

const SPEC_TEMPLATE: &str = r#"# spec.toml テンプレート (`harness init`)
# 詳細は thin-workflow-harness の docs/schemas.md §1 を参照。

[meta]
intent = ""              # 人間が出した変更依頼の一行
status = "draft"         # "draft" | "frozen"

# [[requirement]]
# id = "F-001"
# text = ""
# files = []            # blast radius
# tests = []            # 検証コマンド

# [[acceptance]]
# id = "AC-1"
# requirement = "F-001"
# text = ""
# test = ""

# [[invariant]]
# id = "INV-1"
# text = ""
# test = ""

# [[open_question]]
# id = "Q-1"
# text = ""
# options = []

[approval]
verdict = ""
by = ""
notes = ""
"#;

// 回帰 baseline (state/regression_baseline.json) は蓄積の永続記録なので git 追跡する (明示 negate)。
const GITIGNORE: &str = "state/*.jsonl\nstate/*.questions.jsonl\nstate/*.metrics.jsonl\nstate/*.workflow-snapshot.toml\nstate/*.lock\ntranscripts/\n!state/.gitkeep\n!state/regression_baseline.json\n";

/// skill ファイル。全 12 個を `skill_templates/*.md` から `include_str!` で同梱
/// （fat skills 思想 ── 具体的な tool 呼び方と exit_gates 連携を含む operational
/// template）。harness init で展開された時点で各 skill が実行可能な指示を持つ
/// default workflow が完成する状態。
const SKILL_STUBS: &[(&str, &str)] = &[
    ("01-research.md", include_str!("skill_templates/01-research.md")),
    ("02-plan.md", include_str!("skill_templates/02-plan.md")),
    ("03-characterize.md", include_str!("skill_templates/03-characterize.md")),
    ("04-implement.md", include_str!("skill_templates/04-implement.md")),
    ("05-test.md", include_str!("skill_templates/05-test.md")),
    ("06-security.md", include_str!("skill_templates/06-security.md")),
    ("07-review.md", include_str!("skill_templates/07-review.md")),
    ("08-join.md", include_str!("skill_templates/08-join.md")),
    ("09-docdesign.md", include_str!("skill_templates/09-docdesign.md")),
    ("10-design-pre.md", include_str!("skill_templates/10-design-pre.md")),
    ("11-verify.md", include_str!("skill_templates/11-verify.md")),
    ("12-land.md", include_str!("skill_templates/12-land.md")),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::DetectedProject;

    // 差分 mutation で検出した穴の回帰: write_layout を no-op(Ok(()))化しても誰も気づかなかった。
    // init が差分 mutation ツールと回帰 gate を bin/ に同梱することを固定する。
    #[test]
    fn write_layout_emits_bin_tools() {
        let base = std::env::temp_dir().join(format!("harness-scaffold-test-{}", std::process::id()));
        let harness = base.join(".harness");
        let _ = fs::remove_dir_all(&base);
        write_layout(&harness, &DetectedProject::default()).expect("write_layout failed");
        let bin = harness.join("bin");
        assert!(bin.join("mutate-diff.mjs").exists(), "init が bin/mutate-diff.mjs を生成していない");
        assert!(bin.join("regression_gate.mjs").exists(), "init が bin/regression_gate.mjs を生成していない");
        assert!(bin.join("characterize_gate.mjs").exists(), "init が bin/characterize_gate.mjs を生成していない");
        assert!(bin.join("catalog_gate.mjs").exists(), "init が bin/catalog_gate.mjs を生成していない");
        assert!(bin.join("okf_check.mjs").exists(), "init が bin/okf_check.mjs を生成していない");
        // docs/ 知識バンドルが OKF v0.1 適合形で生成される (予約 index.md/log.md + 概念 doc に type)。
        let repo_root = base.as_path();
        assert!(repo_root.join("docs/index.md").exists(), "init が docs/index.md (OKF 予約) を生成していない");
        assert!(repo_root.join("docs/log.md").exists(), "init が docs/log.md (OKF 予約) を生成していない");
        let ctx = std::fs::read_to_string(repo_root.join("docs/architecture/01-context.md")).unwrap_or_default();
        assert!(ctx.contains("type: architecture-section"), "arch concept doc に OKF 必須 type が無い");
        // 差分 mutation ラチェット baseline / equivalent ledger / catalog waiver の seed。
        let state = harness.join("state");
        assert!(state.join("mutation_baseline.json").exists(), "init が state/mutation_baseline.json を seed していない");
        assert!(state.join("equivalent_mutants.json").exists(), "init が state/equivalent_mutants.json を seed していない");
        assert!(state.join("catalog_waivers.json").exists(), "init が state/catalog_waivers.json を seed していない");
        let _ = fs::remove_dir_all(&base);
    }

    // preservation トラックが 6 tool + 5 skill + テンプレ + golden + state seed を同梱することを固定(no-op 化回帰錨)。
    #[test]
    fn write_preservation_layout_emits_tools_and_seed() {
        let base = std::env::temp_dir().join(format!("harness-preservation-test-{}", std::process::id()));
        let harness = base.join(".harness");
        let _ = fs::remove_dir_all(&base);
        write_preservation_layout(&harness).expect("write_preservation_layout failed");
        let bin = harness.join("bin");
        for t in ["preservation_lib.mjs", "capture_oracle.mjs", "differential_gate.mjs", "reconcile_gate.mjs", "coverage_gate.mjs", "db-assert.mjs"] {
            assert!(bin.join(t).exists(), "preservation init が bin/{t} を生成していない");
        }
        let skills = harness.join("skills");
        for s in ["p01-research.md", "p02-capture.md", "p03-differential.md", "p04-reconcile.md", "p05-coverage.md"] {
            assert!(skills.join(s).exists(), "preservation init が skills/{s} を生成していない");
        }
        assert!(harness.join("equivalence.json").exists(), "equivalence.json 欠落");
        assert!(harness.join("preservation/input_space.json").exists(), "input_space.json 欠落");
        assert!(harness.join("golden/manifest.json").exists(), "golden/manifest.json 欠落");
        let state = harness.join("state");
        for j in ["reconcile_ledger.json", "coverage_ledger.json", "coverage_baseline.json", "capture_provenance.json", "nondeterminism_ledger.json", "absorbed_divergences.json"] {
            assert!(state.join(j).exists(), "preservation init が state/{j} を seed していない");
        }
        let _ = fs::remove_dir_all(&base);
    }
}
