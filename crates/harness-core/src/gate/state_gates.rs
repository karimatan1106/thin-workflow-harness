//! state 系 gate: evidence_recorded / json_has / artifact_registered / count_non_decreasing。

use super::{arg_str, resolve, GateCtx, GateResult};
use crate::state::State;

pub(super) fn evidence_recorded(args: &toml::Table, state: &State) -> GateResult {
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

pub(super) fn json_has(args: &toml::Table, state: &State) -> GateResult {
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

pub(super) fn artifact_registered(args: &toml::Table, state: &State, ctx: &GateCtx) -> GateResult {
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
        let full = resolve(ctx.home, p);
        if !full.is_file() {
            return GateResult::fail(format!("artifact '{n}' のファイル {p} が無い"));
        }
    }
    GateResult::ok(format!("artifact '{key}' {} 件、全て実在", matched.len()))
}

fn num_of(v: &serde_json::Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_f64().map(|f| f as i64))
        .or_else(|| v.get("count").and_then(|c| c.as_i64()))
}

pub(super) fn count_non_decreasing(args: &toml::Table, state: &State) -> GateResult {
    let Some(ekey) = arg_str(args, "evidence_key") else {
        return GateResult::fail("引数 evidence_key が無い");
    };
    let Some(bkey) = arg_str(args, "baseline_key") else {
        return GateResult::fail("引数 baseline_key が無い");
    };
    let Some(cur_v) = state.gate_evidence.get(ekey) else {
        return GateResult::fail(format!("evidence '{ekey}' が未記録"));
    };
    let Some(base_v) = state.gate_evidence.get(bkey) else {
        return GateResult::fail(format!("baseline '{bkey}' が未記録"));
    };
    let Some(cur) = num_of(cur_v) else {
        return GateResult::fail(format!("evidence '{ekey}' が数値でない"));
    };
    let Some(base) = num_of(base_v) else {
        return GateResult::fail(format!("baseline '{bkey}' が数値でない"));
    };
    if cur >= base {
        GateResult::ok(format!("{ekey}={cur} ≥ baseline {base}"))
    } else {
        GateResult::fail(format!("{ekey}={cur} < baseline {base} (減少)"))
    }
}
