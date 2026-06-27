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
        Some("preservation") => {
            scaffold::write_preservation_layout(&harness_dir)?;
            println!(
                "\npreservation(挙動保存=rehost/migration)ワークフローをスキャフォールドしました: {}",
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
            return Err(format!("未知の template '{other}' ── 使えるのは: default, security, preservation"));
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

    // CKG: 検出言語の LSP サーバ + harness-lspd の有無を検査し、無ければ setup-ckg を推奨（install はしない）。
    match detect(&target).lang.as_deref().and_then(|l| ckg_server_for(l).ok().map(|(s, _)| (l.to_string(), s))) {
        Some((lang, server)) => {
            let server_ok = path_has(server);
            let lspd_ok = path_has("harness-lspd");
            if server_ok && lspd_ok {
                println!("[OK] CKG: LSP サーバ {server} + harness-lspd あり (lang={lang})");
            } else {
                warns += 1;
                if !server_ok {
                    println!("[WARN] CKG: LSP サーバ {server} が PATH に無い (lang={lang})");
                }
                if !lspd_ok {
                    println!("[WARN] CKG: harness-lspd が PATH に無い");
                }
                println!("  → `harness setup-ckg` で install (opt-in)");
            }
        }
        None => println!("[OK] CKG: 言語未検出/未対応のためスキップ"),
    }

    println!("\n結果: errors={errors} warns={warns}");
    if errors > 0 {
        Err(format!("{errors} 件のエラー"))
    } else {
        Ok(())
    }
}

/// `harness setup-ckg [<dir>] [--lang ...]` ── CKG コード検索を opt-in でセットアップ。
/// 検出言語の LSP サーバ (rust-analyzer/gopls/...) と harness-lspd を install する。冪等。
/// harness 本体は CKG を「提供」しないが (L0)、別バイナリ harness-lspd の導入を補助する。
pub fn cmd_setup_ckg(dir: Option<&str>, lang_override: Option<&str>) -> Result<(), String> {
    let target = resolve_dir(dir)?;
    let lang = match lang_override {
        Some(l) => l.to_ascii_lowercase(),
        None => detect(&target)
            .lang
            .ok_or_else(|| "言語検出に失敗。--lang <rust|typescript|python|go> を指定してください".to_string())?,
    };
    let (server, install_cmd) = ckg_server_for(&lang)?;
    println!("CKG セットアップ: 言語={lang}");

    // 1. LSP サーバ（無ければ install）。
    if path_has(server) {
        println!("[OK] LSP サーバ {server} は既に PATH にある");
    } else {
        println!("[..] LSP サーバを install: {install_cmd}");
        if !shell_run(install_cmd, &target)? {
            return Err(format!("LSP サーバ install 失敗 ── 手動で実行: {install_cmd}"));
        }
        println!("[OK] {server} install 完了");
    }

    // 2. harness-lspd（無ければ workspace source から cargo install）。
    if path_has("harness-lspd") {
        println!("[OK] harness-lspd は既に PATH にある");
    } else {
        let src = lspd_source_path();
        let cmd = format!("cargo install --path \"{src}\" --bin harness-lspd");
        println!("[..] harness-lspd を install: {cmd}");
        if !shell_run(&cmd, &target)? {
            return Err(format!("harness-lspd install 失敗 ── 手動: {cmd}"));
        }
        println!("[OK] harness-lspd install 完了");
    }

    println!(
        "\n[OK] CKG セットアップ完了。skill から `harness-lspd query symbol|refs|callers|closure|impacted-by|tested-by|outline ...` が使える。"
    );
    Ok(())
}

/// 言語 → (LSP サーバ実行ファイル名, install コマンド)。CKG 対応言語の正典。
fn ckg_server_for(lang: &str) -> Result<(&'static str, &'static str), String> {
    match lang {
        "rust" => Ok(("rust-analyzer", "rustup component add rust-analyzer")),
        "typescript" | "javascript" | "node" => Ok((
            "typescript-language-server",
            "npm i -g typescript-language-server typescript",
        )),
        "python" => Ok(("pyright-langserver", "npm i -g pyright")),
        "go" => Ok(("gopls", "go install golang.org/x/tools/gopls@latest")),
        other => Err(format!("CKG 未対応の言語: {other} (rust|typescript|python|go)")),
    }
}

/// harness-lspd の source パス。ビルド時の manifest dir から workspace の lsp-daemon を導出。
/// `HARNESS_SRC` env で workspace root を上書き可。
fn lspd_source_path() -> String {
    if let Ok(s) = std::env::var("HARNESS_SRC") {
        return format!("{s}/crates/lsp-daemon");
    }
    // CARGO_MANIFEST_DIR = <ws>/crates/harness-core → ../lsp-daemon = <ws>/crates/lsp-daemon
    format!("{}/../lsp-daemon", env!("CARGO_MANIFEST_DIR"))
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
