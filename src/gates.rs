//! 決定論的な gate 評価。

use std::fs;
use std::path::Path;

use crate::state::State;

pub struct GateResult {
    pub ok: bool,
    pub note: String,
}

const FORBIDDEN_WORDS: &[&str] = &[
    "TODO", "TBD", "WIP", "FIXME", "未定", "未確定", "要検討", "検討中", "対応予定", "サンプル", "ダミー", "仮置き",
];

/// ファイルの行数。末尾改行のみは数えない（lines().count() 相当）。
fn count_lines(text: &str) -> usize {
    text.lines().count()
}

fn read_nonempty_file(path: &str) -> Result<String, String> {
    let p = Path::new(path);
    if !p.is_file() {
        return Err(format!("ファイルが存在しません: {path}"));
    }
    let text = fs::read_to_string(p).map_err(|e| format!("読取失敗 {path}: {e}"))?;
    if text.trim().is_empty() {
        return Err(format!("ファイルが空です: {path}"));
    }
    Ok(text)
}

fn pass(note: &str) -> GateResult {
    GateResult { ok: true, note: note.to_string() }
}

fn fail(note: String) -> GateResult {
    GateResult { ok: false, note }
}

/// `impl:` で始まる artifact を (name, path) で列挙。
fn impl_artifacts(state: &State) -> Vec<(&String, &String)> {
    state.artifacts.iter().filter(|(k, _)| k.starts_with("impl:")).collect()
}

pub fn eval_gate(name: &str, state: &State) -> GateResult {
    match name {
        "intent_recorded" => {
            if state.intent.trim().is_empty() {
                fail("intent が記録されていません".into())
            } else {
                pass("intent あり")
            }
        }
        "research_notes_recorded" => match state.artifacts.get("research_notes") {
            None => fail("research_notes が登録されていません".into()),
            Some(p) => match read_nonempty_file(p) {
                Ok(_) => pass("research_notes あり"),
                Err(e) => fail(e),
            },
        },
        "plan_artifact_exists" => match state.artifacts.get("plan") {
            None => fail("plan が登録されていません".into()),
            Some(p) => match read_nonempty_file(p) {
                Ok(_) => pass("plan あり"),
                Err(e) => fail(e),
            },
        },
        "plan_artifact_size_ok" => match state.artifacts.get("plan") {
            None => fail("plan が登録されていません".into()),
            Some(p) => match read_nonempty_file(p) {
                Ok(text) => {
                    let n = count_lines(&text);
                    if n <= 200 {
                        pass(&format!("{n} 行"))
                    } else {
                        fail(format!("{p}: {n} 行 (> 200)"))
                    }
                }
                Err(e) => fail(e),
            },
        },
        "impl_artifacts_exist" => {
            let items = impl_artifacts(state);
            if items.is_empty() {
                return fail("impl: 系の artifact が 0 件です".into());
            }
            for (_, p) in &items {
                if !Path::new(p).is_file() {
                    return fail(format!("ファイルが存在しません: {p}"));
                }
            }
            pass(&format!("{} 件", items.len()))
        }
        "impl_artifacts_size_ok" => {
            let items = impl_artifacts(state);
            if items.is_empty() {
                return fail("impl: 系の artifact が 0 件です".into());
            }
            for (_, p) in &items {
                match read_nonempty_file(p) {
                    Ok(text) => {
                        let n = count_lines(&text);
                        if n > 200 {
                            return fail(format!("{p}: {n} 行 (> 200)"));
                        }
                    }
                    Err(e) => return fail(e),
                }
            }
            pass("全て ≤ 200 行")
        }
        "no_forbidden_words" => {
            let mut targets: Vec<String> = Vec::new();
            if let Some(p) = state.artifacts.get("plan") {
                targets.push(p.clone());
            }
            for (_, p) in impl_artifacts(state) {
                targets.push(p.clone());
            }
            for path in &targets {
                let text = match fs::read_to_string(path) {
                    Ok(t) => t,
                    Err(e) => return fail(format!("読取失敗 {path}: {e}")),
                };
                let upper = text.to_uppercase();
                for w in FORBIDDEN_WORDS {
                    let found = if w.is_ascii() {
                        upper.contains(&w.to_uppercase())
                    } else {
                        text.contains(w)
                    };
                    if found {
                        return fail(format!("{path}: {w}"));
                    }
                }
            }
            pass("禁止語なし")
        }
        "test_result_recorded_and_passing" => match state.gate_evidence.get("test_result") {
            None => fail("test_result が報告されていません".into()),
            Some(v) => {
                let command = v.get("command").and_then(|c| c.as_str());
                let exit_code = v.get("exit_code").and_then(|c| c.as_i64());
                match (command, exit_code) {
                    (Some(_), Some(0)) => pass("exit_code 0"),
                    (Some(_), Some(c)) => fail(format!("exit_code {c} (≠ 0)")),
                    _ => fail("形式不正: {\"command\": String, \"exit_code\": i64} が必要".into()),
                }
            }
        },
        "review_recorded" => match state.gate_evidence.get("review") {
            None => fail("review が報告されていません".into()),
            Some(v) => match v.get("verdict").and_then(|x| x.as_str()) {
                Some("approved") => pass("approved"),
                Some(other) => fail(format!("verdict = {other} (approved ではない)")),
                None => fail("verdict フィールドがありません".into()),
            },
        },
        other => fail(format!("unknown gate: {other}")),
    }
}
