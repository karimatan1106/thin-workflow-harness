//! status / gates 表示のレンダリング。

use crate::gate::GateResult;
use crate::handlers2::{eval_node_gates, RunCtx};
use crate::paths;
use crate::state::State;
use crate::workflow::{current_node, Workflow};

/// `harness status` の出力。
pub fn print_status(wf: &Workflow, st: &State, rc: &RunCtx) {
    println!("run_id : {}", st.run_id);
    println!("intent : {}", st.intent);
    if st.abandoned {
        println!("status : ✗ 放棄済み（abandon）── 以後の遷移はできない");
    }
    if let Some(reason) = &st.stuck {
        println!("stuck  : ⚠ {reason} （人間エスカレーション ── back/abandon/spec 見直しを判断）");
    }
    let n = wf.nodes().len();
    match current_node(wf, st) {
        None => {
            println!("node   : {}/{} ✔ 完了", st.phase_index.min(n), n);
        }
        Some(node) => {
            println!("node   : {}/{} {}", st.phase_index + 1, n, node.display_name());
            if let Some(skill) = &node.skill {
                println!("skill  : {}  (← これを読め)", paths::skill_path(skill).display());
            } else {
                println!("skill  : (なし)");
            }
            let br = node.blast_radius(rc.spec.as_ref());
            if br.is_empty() {
                println!("blast radius: (宣言なし ── ファイル編集制限なし)");
            } else {
                println!("blast radius: {}", br.join(", "));
            }
            if let Some(orj) = &node.on_reject {
                let remaining = orj.after.saturating_sub(rc.reject_streak);
                println!(
                    "reject : {} 回 / 上限 {} （あと {} 回で {}）",
                    rc.reject_streak, orj.after, remaining, orj.goto
                );
            }
            println!("出口 gate:");
            let results = eval_node_gates(wf, node, st, rc);
            print_gate_lines(&results);
        }
    }
    if st.artifacts.is_empty() {
        println!("artifacts: (なし)");
    } else {
        println!("artifacts:");
        for (name, path) in &st.artifacts {
            println!("  {name} -> {path}");
        }
    }
    if st.gate_evidence.is_empty() {
        println!("evidence : (なし)");
    } else {
        let keys: Vec<&str> = st.gate_evidence.keys().map(|s| s.as_str()).collect();
        println!("evidence : {}", keys.join(", "));
    }
    let pending: Vec<&str> = rc
        .questions
        .iter()
        .filter(|q| !q.answered)
        .map(|q| q.id.as_str())
        .collect();
    if !pending.is_empty() {
        println!("質問待ち : {} （`harness questions` 参照）", pending.join(", "));
    }
}

/// gate 評価結果を `[PASS]` / `[FAIL] — reason` で 1 行ずつ。
pub fn print_gate_lines(results: &[(String, GateResult)]) {
    if results.is_empty() {
        println!("  (gate なし)");
        return;
    }
    for (name, r) in results {
        if r.ok {
            println!("  [PASS] {name} — {}", r.note);
        } else {
            println!("  [FAIL] {name} — {}", r.note);
        }
    }
}
