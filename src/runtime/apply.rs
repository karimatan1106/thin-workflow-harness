//! worker のアクションを「対応する harness ハンドラ呼び出し」として適用する。
//!
//! 既存 debug CLI ハンドラ（`handlers` / `handlers_advance` / `handlers3`）のロジックを再利用する
//! ── runtime はオーケストレーションだけを担い、状態遷移と gate 評価は既存経路を通る。

use crate::event::{append_event, EventKind};
use crate::questions::{append_question, next_question_id, QueuedQuestion};
use crate::runtime::interceptor::{Interceptor, Verdict};
use crate::runtime::worker::WorkerAction;
use crate::{handlers, handlers3, handlers_advance};

/// 1 アクションを適用した結果。
pub enum Applied {
    /// 状態を変えない補助アクション（create_file / record_artifact / report_evidence / ask）を適用した。
    Continued,
    /// 遷移を試みた（advance）── このあとは main ループが state を再読込して次を決める。
    Transitioned,
    /// back した ── 同様に main ループが再評価する。
    WentBack,
    /// 自己申告 stuck（escalation 質問を積んだ）── ループは「人間待ち」で終わる。
    Stuck(String),
}

/// アクションを 1 つ適用する。
pub fn apply_action(
    run_id: &str,
    act: &WorkerAction,
    intc: &Interceptor,
) -> Result<Applied, String> {
    match act {
        WorkerAction::CreateFile { path, content } => {
            if let Verdict::Deny(why) = intc.check_write(std::path::Path::new(path)) {
                return Err(format!("インターセプタが書込を拒否: {path} ── {why}"));
            }
            let cwd = std::env::current_dir().map_err(|e| format!("cwd 取得失敗: {e}"))?;
            let full = cwd.join(path);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("ディレクトリ作成失敗 {}: {e}", parent.display()))?;
            }
            std::fs::write(&full, content)
                .map_err(|e| format!("ファイル書込失敗 {}: {e}", full.display()))?;
            Ok(Applied::Continued)
        }
        WorkerAction::RecordArtifact { name, path } => {
            handlers::cmd_record_artifact(name, path, None, Some(run_id))?;
            Ok(Applied::Continued)
        }
        WorkerAction::ReportEvidence { gate, json } => {
            handlers::cmd_report_evidence(gate, json, Some(run_id))?;
            Ok(Applied::Continued)
        }
        WorkerAction::RequestTransition => {
            // 失敗（gate fail / 却下）でも Err を握りつぶし、main ループが state delta で判定する。
            let _ = handlers_advance::cmd_advance(Some(run_id));
            Ok(Applied::Transitioned)
        }
        WorkerAction::Back { reason } => {
            handlers::cmd_back(reason, Some(run_id))?;
            Ok(Applied::WentBack)
        }
        WorkerAction::Ask { question, options, required } => {
            handlers3::cmd_ask(question, options, None, None, *required, Some(run_id))?;
            Ok(Applied::Continued)
        }
        WorkerAction::Stuck { reason } => {
            queue_stuck_escalation(run_id, reason)?;
            Ok(Applied::Stuck(reason.clone()))
        }
    }
}

/// `harness stuck` 相当 ── escalation 質問をキューに積む（`docs/operations.md` §1）。
fn queue_stuck_escalation(run_id: &str, reason: &str) -> Result<(), String> {
    let qid = next_question_id(run_id)?;
    let q = QueuedQuestion {
        id: qid.clone(),
        kind: "escalation".into(),
        question: format!("worker が詰まったと申告: {reason}"),
        header: "stuck".into(),
        options: vec![
            "plan に戻す".into(),
            "gate を見直す".into(),
            "中断".into(),
            "自分でやる".into(),
        ],
        required: true,
        context_ref: None,
    };
    append_question(run_id, q)?;
    append_event(
        run_id,
        EventKind::QuestionQueued { question_id: qid, kind: "escalation".into(), required: true },
    )
    .map_err(|e| format!("question_queued 書込失敗: {e}"))?;
    Ok(())
}

/// run に未回答の必須 escalation 質問があるか（人間待ちの判定）。
pub fn has_pending_escalation(run_id: &str) -> bool {
    crate::questions::read_questions(run_id)
        .map(|qs| qs.iter().any(|q| !q.answered && q.required && q.kind == "escalation"))
        .unwrap_or(false)
}

/// 直近の遷移系イベント以降に `advance_rejected` があるか（再 spawn が要るか）。
pub fn rejected_since_transition(events: &[crate::event::Event]) -> bool {
    let mut rejected = false;
    for ev in events {
        match &ev.kind {
            EventKind::Advance { .. } | EventKind::Back { .. } | EventKind::Reset => rejected = false,
            EventKind::AdvanceRejected { .. } => rejected = true,
            _ => {}
        }
    }
    rejected
}

