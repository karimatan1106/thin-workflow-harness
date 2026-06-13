//! 再発防止 guard test(F-2 / AC-4)── 生成 skill テンプレートが参照する
//! `harness <cmd>` が全て実在 CLI サブコマンドであることを保証する。
//!
//! skill テンプレートは LLM agent への指示書。非実在コマンド(過去の drift:
//! request-transition / stuck / spec / artifact / artifact-list / edit-file)を
//! 参照すると、agent/operator が実行時に clap エラーを踏む。実コマンド集合は
//! `harness --help` の Commands セクションを真実源とし、テンプレ参照との包含を強制する。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// harness-core のソース skill テンプレート .md ディレクトリ。
/// (`include_str!` で bundle される実体と同一ファイル)
fn skill_templates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../harness-core/src/scaffold/skill_templates")
}

/// `harness --help` の Commands セクションから実在サブコマンド名を集める。
fn real_subcommands() -> BTreeSet<String> {
    let o = Command::new(env!("CARGO_BIN_EXE_harness"))
        .arg("--help")
        .output()
        .expect("spawn harness --help");
    let s = String::from_utf8_lossy(&o.stdout);
    let mut cmds = BTreeSet::new();
    let mut in_cmds = false;
    for line in s.lines() {
        if line.trim_start().starts_with("Commands:") {
            in_cmds = true;
            continue;
        }
        if in_cmds {
            if line.trim().is_empty() {
                break;
            }
            if let Some(tok) = line.trim().split_whitespace().next() {
                if !tok.is_empty() && tok.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                    cmds.insert(tok.to_string());
                }
            }
        }
    }
    cmds
}

/// 本文中の `harness <cmd>` 参照(コマンド名トークン)を抽出する。
/// 散文の "harness が"/"harness ランタイム" 等は非 ASCII 小文字で始まるため空になり除外される。
/// `harness-lspd ...` は "harness " (末尾空白) にマッチしないため対象外。
fn referenced_commands(body: &str) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for (i, _) in body.match_indices("harness ") {
        let rest = &body[i + "harness ".len()..];
        let cmd: String = rest
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || *c == '-')
            .collect();
        if cmd.len() >= 2 {
            refs.insert(cmd);
        }
    }
    refs
}

#[test]
fn skill_templates_reference_only_real_subcommands() {
    let real = real_subcommands();
    // sanity: パーサが実コマンドを取れていること
    assert!(
        real.contains("advance") && real.contains("status"),
        "real subcommands not parsed from --help: {real:?}"
    );

    let dir = skill_templates_dir();
    let mut violations: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    for entry in std::fs::read_dir(&dir).expect("read skill_templates dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        scanned += 1;
        let body = std::fs::read_to_string(&path).unwrap();
        let fname = path.file_name().unwrap().to_string_lossy().to_string();
        for cmd in referenced_commands(&body) {
            if cmd == "help" {
                continue; // `harness help` / `--help` は常に存在
            }
            if !real.contains(&cmd) {
                violations.push(format!("{fname}: `harness {cmd}` は実在サブコマンドでない"));
            }
        }
    }
    assert!(scanned >= 10, "skill テンプレが想定数読めていない: {scanned}");
    assert!(
        violations.is_empty(),
        "skill テンプレが非実在 harness コマンドを参照している(ドリフト):\n{}",
        violations.join("\n")
    );
}
