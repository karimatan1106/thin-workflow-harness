//! `.harness/` レイアウトのスキャフォールド ── workflow.toml / skills / spec.toml /
//! state/.gitkeep / .gitignore を生成する。
//!
//! `docs/onboarding.md` §3 ／ `docs/schemas.md` §2.2「デフォルトワークフローの例」準拠。
//! skill 文面の同梱方法は実装で確定するため、ここではプレースホルダ＋参照案内のみ。

mod workflow_tmpl;

use std::fs;
use std::path::Path;

use crate::detect::DetectedProject;

/// `.harness/` を target に丸ごと書き出す（既存なら上書き）。
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
    for (name, body) in SKILL_STUBS {
        write_file(&skills.join(name), body)?;
    }
    Ok(())
}

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

const GITIGNORE: &str = "state/*.jsonl\nstate/*.questions.jsonl\nstate/*.metrics.jsonl\nstate/*.workflow-snapshot.toml\ntranscripts/\n!state/.gitkeep\n";

/// skill ファイル。research / plan / test の 3 つは `skill_templates/*.md` から
/// `include_str!` で同梱（fat skills 思想 ── 具体的な tool 呼び方と exit_gates 連携
/// を含む operational template）。残り 5 つ（characterize / implement / security /
/// review / join）は docs/skill-templates.md への参照スタブ（次バッチで充実予定）。
const SKILL_STUBS: &[(&str, &str)] = &[
    ("01-research.md", include_str!("skill_templates/01-research.md")),
    ("02-plan.md", include_str!("skill_templates/02-plan.md")),
    ("03-characterize.md", "# characterize skill\n\nこのノードの skill を記述する。標準文面は thin-workflow-harness の docs/skill-templates.md の `## skill: characterize` を参照（プロジェクトに合わせて調整）。\n"),
    ("04-implement.md", "# implement skill\n\nこのノードの skill を記述する。標準文面は thin-workflow-harness の docs/skill-templates.md の `## skill: implement` を参照（プロジェクトに合わせて調整）。\n"),
    ("05-test.md", include_str!("skill_templates/05-test.md")),
    ("06-security.md", "# security skill\n\nこのノードの skill を記述する。標準文面は thin-workflow-harness の docs/skill-templates.md の `## skill: security` を参照（プロジェクトに合わせて調整）。\n"),
    ("07-review.md", "# review skill\n\nこのノードの skill を記述する。標準文面は thin-workflow-harness の docs/skill-templates.md の `## skill: review` を参照（プロジェクトに合わせて調整）。\n"),
    ("08-join.md", "# join skill\n\n並列ブランチをマージし再検証するノード。標準文面は thin-workflow-harness の docs/skill-templates.md の `## skill: join` を参照（プロジェクトに合わせて調整）。\n"),
];
