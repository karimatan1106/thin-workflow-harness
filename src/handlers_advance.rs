//! `advance` コマンド（出口 gate 評価 + on_reject 遷移）。

use crate::event::{append_event, read_events, Event, EventKind, FailedGate};
use crate::handlers::{ensure_active, load_wf, show, state_for};
use crate::handlers2::{eval_node_gates, RunCtx};
use crate::paths;
use crate::questions::{append_question, next_question_id, QueuedQuestion};
use crate::workflow::current_node;

/// 現ノードに留まっている間の `advance_rejected` 連続回数（最後の遷移/リセット以降）。
fn reject_streak(events: &[Event]) -> usize {
    let mut count = 0;
    for ev in events {
        match &ev.kind {
            EventKind::Advance { .. } | EventKind::Back { .. } | EventKind::Reset => count = 0,
            EventKind::AdvanceRejected { .. } => count += 1,
            _ => {}
        }
    }
    count
}

pub fn cmd_advance(run: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let wf = load_wf()?;
    let st = state_for(&run_id, &wf)?;
    ensure_active(&st)?;
    let Some(node) = current_node(&wf, &st) else {
        return Err("既に完了している".to_string());
    };
    let rc = RunCtx::load(&run_id);
    let results = eval_node_gates(&wf, node, &st, &rc);
    let failed: Vec<FailedGate> = results
        .iter()
        .filter(|(_, r)| !r.ok)
        .map(|(g, r)| FailedGate { gate: g.clone(), reason: r.note.clone() })
        .collect();
    let from = node.id.clone();
    if failed.is_empty() {
        let to = wf
            .nodes()
            .get(st.phase_index + 1)
            .map(|n| n.id.clone())
            .unwrap_or_else(|| "(done)".to_string());
        append_event(&run_id, EventKind::Advance { from, to: to.clone() })
            .map_err(|e| format!("advance 書込失敗: {e}"))?;
        println!("advance → {to}");
        return show(&wf, &run_id);
    }
    append_event(&run_id, EventKind::AdvanceRejected { failed_gates: failed.clone() })
        .map_err(|e| format!("advance_rejected 書込失敗: {e}"))?;
    eprintln!("却下: {} 個の gate が fail", failed.len());
    for f in &failed {
        eprintln!("  [FAIL] {} — {}", f.gate, f.reason);
    }
    if let Some(orj) = &node.on_reject {
        let events = read_events(&run_id)?;
        let streak = reject_streak(&events);
        if streak >= orj.after {
            if orj.goto == "__human__" {
                escalate(&run_id, &node.id, streak)?;
                return Err("人間の判断待ち（`harness questions` 参照）".to_string());
            }
            jump_to(&wf, &run_id, st.phase_index, &orj.goto)?;
            println!("on_reject: {streak} 回 reject → {} へ遷移", orj.goto);
            return show(&wf, &run_id);
        }
    }
    Err("advance 却下".to_string())
}

/// phase_index を `goto` ノードに合わせる ── 差分を back/advance イベント列で表す。
fn jump_to(
    wf: &crate::workflow::Workflow,
    run_id: &str,
    cur: usize,
    goto: &str,
) -> Result<(), String> {
    let Some(idx) = wf.index_of(goto) else {
        return Err(format!("on_reject.goto '{goto}' が実在ノードでない"));
    };
    if idx < cur {
        for _ in 0..(cur - idx) {
            append_event(run_id, EventKind::Back { reason: format!("on_reject → {goto}") })
                .map_err(|e| format!("back 書込失敗: {e}"))?;
        }
    } else {
        let mut at = cur;
        while at < idx {
            let f = wf.nodes()[at].id.clone();
            let t = wf.nodes()[at + 1].id.clone();
            append_event(run_id, EventKind::Advance { from: f, to: t })
                .map_err(|e| format!("advance 書込失敗: {e}"))?;
            at += 1;
        }
    }
    Ok(())
}

fn escalate(run_id: &str, node_id: &str, streak: usize) -> Result<(), String> {
    let qid = next_question_id(run_id)?;
    let q = QueuedQuestion {
        id: qid.clone(),
        kind: "escalation".into(),
        question: format!("ノード '{node_id}' が {streak} 回 reject されました。どうしますか?"),
        header: "エスカレ".into(),
        options: vec!["plan に戻す".into(), "このノードの gate を見直す".into(), "中断".into()],
        required: true,
        context_ref: Some(node_id.to_string()),
    };
    append_question(run_id, q)?;
    append_event(
        run_id,
        EventKind::QuestionQueued { question_id: qid, kind: "escalation".into(), required: true },
    )
    .map_err(|e| format!("question_queued 書込失敗: {e}"))?;
    eprintln!("人間エスカレ質問をキューに積みました（`harness questions`）");
    Ok(())
}
