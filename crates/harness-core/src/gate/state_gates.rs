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

/// evidence の json_path が「実体のある値」かを検証する。
/// 空文字 / 空配列 / 空オブジェクト / null は fail。`evidence_recorded`(記録の有無) や
/// `json_has`(文字列一致) では塞げない「中身が空の逃げ」を防ぐための gate。
/// 例: review の master_design_update で `architecture_sections_changed` が空配列なら fail。
pub(super) fn json_nonempty(args: &toml::Table, state: &State) -> GateResult {
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
    let empty = match found {
        serde_json::Value::Null => true,
        serde_json::Value::String(s) => s.trim().is_empty(),
        serde_json::Value::Array(a) => a.is_empty(),
        serde_json::Value::Object(o) => o.is_empty(),
        _ => false, // 数値 / bool は「実体あり」とみなす
    };
    if empty {
        GateResult::fail(format!("{ekey}.{jpath} が空 (実体のある内容が必要)"))
    } else {
        GateResult::ok(format!("{ekey}.{jpath} に実体あり"))
    }
}

/// evidence の json_path の値が許可リスト(`one_of`, カンマ区切り)に入っているかを検証する。
/// verdict の値域を縛り、`no_change` のような skill 未定義の逃げ値を排除する。
/// 例: master_design_update.verdict が "updated"/"noop" のいずれかであることを強制。
pub(super) fn json_in(args: &toml::Table, state: &State) -> GateResult {
    let Some(ekey) = arg_str(args, "evidence_key") else {
        return GateResult::fail("引数 evidence_key が無い");
    };
    let Some(jpath) = arg_str(args, "json_path") else {
        return GateResult::fail("引数 json_path が無い");
    };
    let Some(allowed) = arg_str(args, "one_of") else {
        return GateResult::fail("引数 one_of (カンマ区切り) が無い");
    };
    let Some(data) = state.gate_evidence.get(ekey) else {
        return GateResult::fail(format!("evidence '{ekey}' が未記録"));
    };
    let Some(found) = json_path_get(data, jpath) else {
        return GateResult::fail(format!("evidence '{ekey}' に json_path '{jpath}' が無い"));
    };
    let got = found.as_str().map(|s| s.to_string()).unwrap_or_else(|| found.to_string());
    let allow: Vec<&str> = allowed.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if allow.iter().any(|a| *a == got) {
        GateResult::ok(format!("{ekey}.{jpath} = {got} (許可値)"))
    } else {
        GateResult::fail(format!("{ekey}.{jpath} = {got} は許可値 {allow:?} に無い"))
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
