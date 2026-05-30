//! `harness init` / `harness doctor` ── onboarding と健全性チェック。
//!
//! - `init` は `<dir>` を検出 → `.harness/` スキャフォールド → `harness validate` → スモーク。
//! - `doctor` は `.harness/` を検査し `[OK] / [WARN] X / [ERROR] X` を一覧する（自動修正なし）。

mod util;

use std::path::{Path, PathBuf};

use crate::detect::detect;
use crate::scaffold;
use crate::workflow::{load_workflow, validate, GateSpec};

use util::{path_has, print_summary, resolve_dir, resolve_harness_dir, shell_run};

/// `harness init [<dir>] [--force] [--template <name>]` の実装。
/// template 省略/`default` → 検出ベースの標準ワークフロー。
/// `security` → security-only ワークフロー（検出不要・単一ノード）。
pub fn cmd_init(dir: Option<&str>, force: bool, template: Option<&str>) -> Result<(), String> {
    let target = resolve_dir(dir)?;
    println!("検出ディレクトリ: {}", target.display());
    let harness_dir = target.join(".harness");
    if harness_dir.exists() && !force {
        return Err(format!(
            "{} は既に存在する。--force で上書き可",
            harness_dir.display()
        ));
    }

    // 検出ベースの default のみ smoke check で `detected` を使う。security は None。
    let detected = match template {
        Some("security") => {
            scaffold::write_security_layout(&harness_dir)?;
            println!(
                "\nsecurity-only ワークフローをスキャフォールドしました: {}",
                harness_dir.display()
            );
            None
        }
        None | Some("default") => {
            let d = detect(&target);
            print_summary(&d);
            scaffold::write_layout(&harness_dir, &d)?;
            println!("\n.harness/ をスキャフォールドしました: {}", harness_dir.display());
            Some(d)
        }
        Some(other) => {
            return Err(format!("未知の template '{other}' ── 使えるのは: default, security"));
        }
    };

    let wf_path = harness_dir.join("workflow.toml");
    match load_workflow(&wf_path) {
        Ok(wf) => {
            let errs = validate(&wf, None);
            if errs.is_empty() {
                println!("[OK] harness validate: {} ノード", wf.nodes().len());
            } else {
                println!("[WARN] harness validate: {} 件のエラー", errs.len());
                for e in errs {
                    println!("  - {e}");
                }
            }
        }
        Err(e) => println!("[WARN] workflow.toml ロード失敗: {e}"),
    }

    // smoke は検出ベース default のみ（security テンプレートは build/check を持たない）。
    if let Some(d) = detected.as_ref() {
        if let Some(cmd) = d.check.as_deref().or(d.build.as_deref()) {
            println!("\nスモーク: {cmd}");
            match shell_run(cmd, &target) {
                Ok(true) => println!("[OK] スモーク exit 0"),
                Ok(false) => println!(
                    "[WARN] スモーク exit≠0 ── clean checkout で {cmd} が通らない。検出を確認してください"
                ),
                Err(e) => println!("[WARN] スモーク実行失敗: {e}"),
            }
        } else {
            println!("\n[WARN] build / check コマンドが検出できなかった ── workflow.toml の cmd_exit_0 を編集してください");
        }
    }

    println!(
        "\n次の手順: .harness/workflow.toml を確認・編集 → `harness start \"...\"`（CWD={} から auto-detect）\n  workspace 切替や explicit 指定は HARNESS_HOME={} を設定",
        harness_dir.parent().map(|p| p.display().to_string()).unwrap_or_else(|| ".".to_string()),
        harness_dir.display()
    );
    Ok(())
}

/// `harness doctor [<dir>] [--full]` の実装。
pub fn cmd_doctor(dir: Option<&str>, full: bool) -> Result<(), String> {
    let harness_dir = resolve_harness_dir(dir)?;
    println!("doctor 対象: {}", harness_dir.display());
    let mut warns = 0usize;
    let mut errors = 0usize;
    let target = harness_dir.parent().unwrap_or(Path::new(".")).to_path_buf();

    let wf_path = harness_dir.join("workflow.toml");
    let wf = match load_workflow(&wf_path) {
        Ok(wf) => {
            let errs = validate(&wf, None);
            if errs.is_empty() {
                println!("[OK] validate: {} ノード", wf.nodes().len());
            } else {
                errors += errs.len();
                println!("[ERROR] validate: {} 件", errs.len());
                for e in errs {
                    println!("  - {e}");
                }
            }
            Some(wf)
        }
        Err(e) => {
            errors += 1;
            println!("[ERROR] workflow.toml ロード失敗: {e}");
            None
        }
    };

    if let Some(wf) = &wf {
        let skills_dir = harness_dir.join("skills");
        for n in wf.nodes() {
            if let Some(skill) = &n.skill {
                let sp = skills_dir.join(skill);
                if !sp.exists() {
                    warns += 1;
                    println!("[WARN] ノード '{}' の skill ファイル欠落: {}", n.id, sp.display());
                } else {
                    println!("[OK] skill 存在: {}", sp.display());
                }
            }
        }
        for gs in wf.meta.mandatory_gates.iter() {
            if gs.gate == "cmd_exit_0" {
                check_cmd_gate(gs, &target, full, &mut warns);
            }
        }
        for n in wf.nodes() {
            for gs in &n.exit_gates {
                if gs.gate == "cmd_exit_0" {
                    check_cmd_gate(gs, &target, full, &mut warns);
                }
            }
        }
    }

    println!("[OK] CKG 未設定 (Phase 1.5 で導入予定)");

    println!("\n結果: errors={errors} warns={warns}");
    if errors > 0 {
        Err(format!("{errors} 件のエラー"))
    } else {
        Ok(())
    }
}

fn check_cmd_gate(gs: &GateSpec, cwd: &Path, full: bool, warns: &mut usize) {
    let args = gs.args_table();
    let Some(cmd) = args.get("cmd").and_then(|v| v.as_str()) else { return };
    let first = cmd.split_whitespace().next().unwrap_or("");
    if first.is_empty() {
        return;
    }
    if first == "false" || first == "echo" {
        *warns += 1;
        println!("[WARN] gate cmd プレースホルダ: {cmd}");
        return;
    }
    let prog = first.trim_start_matches("./");
    if !path_has(prog) {
        *warns += 1;
        println!("[WARN] gate cmd の実行ファイルが PATH に無い: {prog}");
        return;
    }
    let heavy = cmd.contains("test") || cmd.contains("e2e") || cmd.contains("coverage");
    if heavy && !full {
        println!("[OK] gate cmd 実行ファイル存在 (実行スキップ heavy): {prog}");
        return;
    }
    match shell_run(cmd, cwd) {
        Ok(true) => println!("[OK] gate cmd exit 0: {cmd}"),
        Ok(false) => {
            *warns += 1;
            println!("[WARN] gate cmd exit≠0: {cmd}");
        }
        Err(e) => {
            *warns += 1;
            println!("[WARN] gate cmd 実行失敗 ({e}): {cmd}");
        }
    }
}

// ── re-export for tests ────────────────────────────────────────────────────
#[allow(dead_code)]
pub(crate) fn _internal_resolve_dir(s: Option<&str>) -> Result<PathBuf, String> {
    resolve_dir(s)
}
