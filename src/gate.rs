//! gate プリミティブ（Phase 0 walking skeleton ── 6 個のみ）。
//!
//! 各 gate は `(state) -> (ok, note)` の純粋関数。未知の名前は `ok=false`。
//! 残り 10 個（max_lines / traceability_closed / workflow_append_only 等）は後フェーズ。

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::state::State;

/// gate 評価の戻り値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateResult {
    pub ok: bool,
    pub note: String,
}

impl GateResult {
    fn ok(note: impl Into<String>) -> Self {
        GateResult { ok: true, note: note.into() }
    }
    fn fail(note: impl Into<String>) -> Self {
        GateResult { ok: false, note: note.into() }
    }
}

/// gate 評価の文脈（相対パス解決の基準など）。
pub struct GateCtx<'a> {
    pub home: &'a Path,
}

/// Phase 0 で実装済みの gate プリミティブ名一覧。
pub fn known_gates() -> &'static [&'static str] {
    &[
        "file_exists",
        "file_nonempty",
        "cmd_exit_0",
        "evidence_recorded",
        "json_has",
        "artifact_registered",
    ]
}

fn arg_str<'a>(args: &'a toml::Table, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn resolve(ctx: &GateCtx, p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        pb
    } else {
        ctx.home.join(pb)
    }
}

/// gate を評価する。
pub fn eval_gate(name: &str, args: &toml::Table, state: &State, ctx: &GateCtx) -> GateResult {
    match name {
        "file_exists" => gate_file_exists(args, ctx),
        "file_nonempty" => gate_file_nonempty(args, ctx),
        "cmd_exit_0" => gate_cmd_exit_0(args, ctx),
        "evidence_recorded" => gate_evidence_recorded(args, state),
        "json_has" => gate_json_has(args, state),
        "artifact_registered" => gate_artifact_registered(args, state, ctx),
        other => GateResult::fail(format!("unknown gate: {other}")),
    }
}

fn gate_file_exists(args: &toml::Table, ctx: &GateCtx) -> GateResult {
    let Some(p) = arg_str(args, "path") else {
        return GateResult::fail("引数 path が無い");
    };
    let full = resolve(ctx, p);
    if full.is_file() {
        GateResult::ok(format!("{p} が存在"))
    } else {
        GateResult::fail(format!("{p} が無い"))
    }
}

fn gate_file_nonempty(args: &toml::Table, ctx: &GateCtx) -> GateResult {
    let Some(p) = arg_str(args, "path") else {
        return GateResult::fail("引数 path が無い");
    };
    let full = resolve(ctx, p);
    match std::fs::metadata(&full) {
        Ok(m) if m.is_file() && m.len() > 0 => GateResult::ok(format!("{p} は非空 ({} bytes)", m.len())),
        Ok(m) if m.is_file() => GateResult::fail(format!("{p} は空")),
        _ => GateResult::fail(format!("{p} が無い")),
    }
}

fn gate_cmd_exit_0(args: &toml::Table, ctx: &GateCtx) -> GateResult {
    let Some(cmd) = arg_str(args, "cmd") else {
        return GateResult::fail("引数 cmd が無い");
    };
    let mut c = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(cmd);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmd);
        c
    };
    c.current_dir(ctx.home);
    match c.output() {
        Ok(out) => {
            let code = out.status.code();
            let mut tail = String::from_utf8_lossy(&out.stderr).to_string();
            if tail.trim().is_empty() {
                tail = String::from_utf8_lossy(&out.stdout).to_string();
            }
            let snippet: String = tail.lines().take(3).collect::<Vec<_>>().join(" | ");
            if out.status.success() {
                GateResult::ok(format!("`{cmd}` exit 0"))
            } else {
                GateResult::fail(format!("`{cmd}` exit {code:?}: {snippet}"))
            }
        }
        Err(e) => GateResult::fail(format!("`{cmd}` 実行失敗: {e}")),
    }
}

fn gate_evidence_recorded(args: &toml::Table, state: &State) -> GateResult {
    let Some(key) = arg_str(args, "key") else {
        return GateResult::fail("引数 key が無い");
    };
    if state.gate_evidence.contains_key(key) {
        GateResult::ok(format!("evidence '{key}' あり"))
    } else {
        GateResult::fail(format!("evidence '{key}' が未記録"))
    }
}

/// ドット区切りパスで JSON を辿る。
fn json_path_get<'a>(v: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut cur = v;
    for seg in path.split('.') {
        if seg.is_empty() {
            continue;
        }
        cur = cur.get(seg)?;
    }
    Some(cur)
}

fn gate_json_has(args: &toml::Table, state: &State) -> GateResult {
    let Some(ekey) = arg_str(args, "evidence_key") else {
        return GateResult::fail("引数 evidence_key が無い");
    };
    let Some(jpath) = arg_str(args, "json_path") else {
        return GateResult::fail("引数 json_path が無い");
    };
    let Some(data) = state.gate_evidence.get(ekey) else {
        return GateResult::fail(format!("evidence '{ekey}' が未記録"));
    };
    let Some(found) = json_path_get(data, jpath) else {
        return GateResult::fail(format!("evidence '{ekey}' に json_path '{jpath}' が無い"));
    };
    if let Some(eq) = args.get("eq") {
        let want = eq.as_str().map(|s| s.to_string()).unwrap_or_else(|| eq.to_string());
        let got = found.as_str().map(|s| s.to_string()).unwrap_or_else(|| found.to_string());
        if got == want {
            GateResult::ok(format!("{ekey}.{jpath} == {want}"))
        } else {
            GateResult::fail(format!("{ekey}.{jpath} = {got}, 期待 {want}"))
        }
    } else {
        GateResult::ok(format!("{ekey}.{jpath} あり"))
    }
}

fn gate_artifact_registered(args: &toml::Table, state: &State, ctx: &GateCtx) -> GateResult {
    let key = arg_str(args, "name_or_prefix").or_else(|| arg_str(args, "name"));
    let Some(key) = key else {
        return GateResult::fail("引数 name_or_prefix / name が無い");
    };
    let matched: Vec<(&String, &String)> = state
        .artifacts
        .iter()
        .filter(|(n, _)| n.as_str() == key || (key.ends_with(':') && n.starts_with(key)))
        .collect();
    if matched.is_empty() {
        return GateResult::fail(format!("artifact '{key}' が未登録"));
    }
    for (n, p) in &matched {
        let full = resolve(ctx, p);
        if !full.is_file() {
            return GateResult::fail(format!("artifact '{n}' のファイル {p} が無い"));
        }
    }
    GateResult::ok(format!("artifact '{key}' {} 件、全て実在", matched.len()))
}
