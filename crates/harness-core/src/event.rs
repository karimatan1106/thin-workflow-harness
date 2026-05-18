//! イベント型と append-only jsonl ログの read/write。
//!
//! 各行 1 JSON、共通フィールド `ts`（ISO8601 UTC）。`derive_state` は純粋 fold。

use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::paths;

/// gate fail の 1 件（`advance_rejected` payload）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailedGate {
    pub gate: String,
    pub reason: String,
}

/// イベント payload（type タグ付き）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    Start {
        intent: String,
    },
    Advance {
        from: String,
        to: String,
    },
    AdvanceRejected {
        failed_gates: Vec<FailedGate>,
    },
    Back {
        reason: String,
    },
    Artifact {
        name: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tag: Option<String>,
    },
    GateEvidence {
        gate: String,
        data: serde_json::Value,
    },
    Reset,
    QuestionQueued {
        question_id: String,
        kind: String,
        required: bool,
    },
    HumanAnswer {
        question_id: String,
        answer: String,
    },
    Abandon {
        reason: String,
    },
    /// fork ノードが branches を spawn したマーカー（ペアになる BranchJoined がいずれ来る）。
    BranchForked {
        branch_ids: Vec<String>,
    },
    /// fork で spawn した全 branch が完了した（status="success"）か失敗した（status="failed"）。
    BranchJoined {
        branch_ids: Vec<String>,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        failures: Option<Vec<String>>,
    },
}

/// 1 イベント（`ts` + payload）。jsonl の 1 行に対応。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub ts: String,
    #[serde(flatten)]
    pub kind: EventKind,
}

impl Event {
    /// 現在時刻を `ts` にして payload を包む。
    pub fn now(kind: EventKind) -> Self {
        Event {
            ts: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            kind,
        }
    }
}

/// run のイベントログに 1 行追記する（ts を付ける）。
pub fn append_event(run_id: &str, kind: EventKind) -> io::Result<()> {
    let path = paths::event_log_path(run_id).map_err(io::Error::other)?;
    let ev = Event::now(kind);
    let line = serde_json::to_string(&ev)?;
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "{line}")?;
    Ok(())
}

/// branch のサブイベントログに 1 行追記する（fork 内 thread が使う）。
pub fn append_branch_event(run_id: &str, branch_id: &str, kind: EventKind) -> io::Result<()> {
    let path = paths::branch_log_path(run_id, branch_id).map_err(io::Error::other)?;
    let ev = Event::now(kind);
    let line = serde_json::to_string(&ev)?;
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "{line}")?;
    Ok(())
}

/// branch のサブイベントログを読む（fold-in 用）。ファイル無しなら空 Vec。
pub fn read_branch_events(run_id: &str, branch_id: &str) -> Result<Vec<Event>, String> {
    let path = paths::branch_log_path(run_id, branch_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let f = std::fs::File::open(&path).map_err(|e| format!("branch ログ読取失敗 {}: {e}", path.display()))?;
    let reader = BufReader::new(f);
    let mut events = Vec::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("branch ログ 行 {} 読取失敗: {e}", i + 1))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let ev: Event = serde_json::from_str(line)
            .map_err(|e| format!("branch ログ 行 {} の JSON パース失敗: {e}", i + 1))?;
        events.push(ev);
    }
    Ok(events)
}

/// run のイベントログを読む。ファイル無しなら空 Vec、壊れた行はエラー。
pub fn read_events(run_id: &str) -> Result<Vec<Event>, String> {
    let path = paths::event_log_path(run_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let f = std::fs::File::open(&path).map_err(|e| format!("イベントログ読取失敗 {}: {e}", path.display()))?;
    let reader = BufReader::new(f);
    let mut events = Vec::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("行 {} 読取失敗: {e}", i + 1))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let ev: Event = serde_json::from_str(line)
            .map_err(|e| format!("行 {} の JSON パース失敗: {e}", i + 1))?;
        events.push(ev);
    }
    Ok(events)
}
