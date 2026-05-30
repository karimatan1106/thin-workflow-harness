//! ファイル系 gate: file_exists / file_nonempty / max_lines / lines_not_increased / no_regex / cmd_exit_0。

use std::process::Command;

use super::{arg_bool, arg_i64, arg_str, glob_paths, GateCtx, GateResult};
use crate::state::State;

pub(super) fn file_exists(args: &toml::Table, ctx: &GateCtx) -> GateResult {
    let Some(p) = arg_str(args, "path") else {
        return GateResult::fail("引数 path が無い");
    };
    let hits = glob_paths(ctx.home, p);
    if hits.iter().any(|f| f.is_file()) {
        GateResult::ok(format!("{p} が存在"))
    } else {
        GateResult::fail(format!("{p} が無い"))
    }
}

pub(super) fn file_nonempty(args: &toml::Table, ctx: &GateCtx) -> GateResult {
    let Some(p) = arg_str(args, "path") else {
        return GateResult::fail("引数 path が無い");
    };
    let hits = glob_paths(ctx.home, p);
    let mut found = false;
    for f in &hits {
        match std::fs::metadata(f) {
            Ok(m) if m.is_file() => {
                found = true;
                if m.len() == 0 {
                    return GateResult::fail(format!("{} は空", f.display()));
                }
            }
            _ => {}
        }
    }
    if found {
        GateResult::ok(format!("{p} は非空"))
    } else {
        GateResult::fail(format!("{p} が無い"))
    }
}

fn count_lines(path: &std::path::Path) -> Option<usize> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(text.lines().count())
}

pub(super) fn max_lines(args: &toml::Table, ctx: &GateCtx) -> GateResult {
    let Some(p) = arg_str(args, "path") else {
        return GateResult::fail("引数 path が無い");
    };
    let Some(n) = arg_i64(args, "n") else {
        return GateResult::fail("引数 n が無い");
    };
    let allow_empty = arg_bool(args, "allow_empty").unwrap_or(false);
    let n = n.max(0) as usize;
    let hits = glob_paths(ctx.home, p);
    let mut checked = 0;
    let mut worst = 0;
    for f in &hits {
        if let Some(c) = count_lines(f) {
            checked += 1;
            worst = worst.max(c);
            if c > n {
                return GateResult::fail(format!("{} は {c} 行 (上限 {n})", f.display()));
            }
        }
    }
    if checked == 0 {
        if allow_empty {
            return GateResult::ok(format!("{p} 該当 0 件 (allow_empty)"));
        }
        return GateResult::fail(format!("{p} に該当ファイルが無い"));
    }
    GateResult::ok(format!("{p} 全 {checked} 件 ≤ {n} 行 (最大 {worst})"))
}

pub(super) fn lines_not_increased(args: &toml::Table, st: &State, ctx: &GateCtx) -> GateResult {
    let Some(p) = arg_str(args, "path") else {
        return GateResult::fail("引数 path が無い");
    };
    let Some(bkey) = arg_str(args, "baseline_key") else {
        return GateResult::fail("引数 baseline_key が無い");
    };
    let Some(base_v) = st.gate_evidence.get(bkey) else {
        return GateResult::fail(format!("baseline evidence '{bkey}' が未記録"));
    };
    let baseline = base_v
        .as_i64()
        .or_else(|| base_v.get("lines").and_then(|v| v.as_i64()))
        .unwrap_or(i64::MAX) as usize;
    let hits = glob_paths(ctx.home, p);
    let mut total = 0usize;
    let mut found = false;
    for f in &hits {
        if let Some(c) = count_lines(f) {
            found = true;
            total += c;
        }
    }
    if !found {
        return GateResult::fail(format!("{p} が無い"));
    }
    if total <= baseline {
        GateResult::ok(format!("{p} は {total} 行 (baseline {baseline} 以下)"))
    } else {
        GateResult::fail(format!("{p} は {total} 行 — baseline {baseline} を超過"))
    }
}

pub(super) fn no_regex(args: &toml::Table, ctx: &GateCtx) -> GateResult {
    let Some(p) = arg_str(args, "path") else {
        return GateResult::fail("引数 path が無い");
    };
    let Some(pattern) = arg_str(args, "pattern") else {
        return GateResult::fail("引数 pattern が無い");
    };
    // regex crate を増やさず、`|` 区切りのリテラル代替（禁止語リスト用途で十分）。
    let needles: Vec<&str> = pattern.split('|').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let hits = glob_paths(ctx.home, p);
    let mut scanned = 0;
    for f in &hits {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        scanned += 1;
        for needle in &needles {
            if text.contains(needle) {
                return GateResult::fail(format!("{} に禁止パターン '{needle}'", f.display()));
            }
        }
    }
    GateResult::ok(format!("{p} 全 {scanned} 件に '{pattern}' なし"))
}

pub(super) fn cmd_exit_0(args: &toml::Table, ctx: &GateCtx) -> GateResult {
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
