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

/// security-only テンプレート（バイナリ同梱 ── 旧 harness-plugin-security の中身）。
const SECURITY_WORKFLOW: &str = include_str!("templates/security_workflow.toml");
const SECURITY_SKILL: &str = include_str!("templates/security_skill.md");

/// 回帰 gate ツール（言語非依存・config 駆動。`tool_templates/regression_gate.mjs` を同梱）。
const REGRESSION_GATE_MJS: &str = include_str!("tool_templates/regression_gate.mjs");

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

/// skill ファイル。全 10 個を `skill_templates/*.md` から `include_str!` で同梱
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
];
