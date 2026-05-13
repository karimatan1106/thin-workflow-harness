//! 質問キュー（`state/<run-id>.questions.jsonl`、append-only）。
//!
//! 各行は `{"op":"add","question":{...}}` か `{"op":"answer","id":"...","answer":"..."}`。
//! `read_questions` が fold して `Question`（answered フラグ込み）に畳む。

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::gate::Question;
use crate::paths;

/// キューに積む質問の素データ（id は呼び出し側で採番）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueuedQuestion {
    pub id: String,
    pub kind: String,
    pub question: String,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub context_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum QLine {
    Add {
        ts: String,
        question: QueuedQuestion,
    },
    Answer {
        ts: String,
        id: String,
        answer: String,
    },
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn queue_path(run_id: &str) -> Result<std::path::PathBuf, String> {
    Ok(paths::state_dir()?.join(format!("{run_id}.questions.jsonl")))
}

fn append_line(run_id: &str, line: &QLine) -> Result<(), String> {
    let path = queue_path(run_id)?;
    let s = serde_json::to_string(line).map_err(|e| format!("質問行のシリアライズ失敗: {e}"))?;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("質問キュー書込失敗 {}: {e}", path.display()))?;
    writeln!(f, "{s}").map_err(|e| format!("質問キュー書込失敗: {e}"))?;
    Ok(())
}

/// 質問を 1 件積む。
pub fn append_question(run_id: &str, q: QueuedQuestion) -> Result<(), String> {
    append_line(run_id, &QLine::Add { ts: now(), question: q })
}

/// 回答を 1 件記録する。
pub fn append_answer(run_id: &str, id: &str, answer: &str) -> Result<(), String> {
    append_line(run_id, &QLine::Answer { ts: now(), id: id.to_string(), answer: answer.to_string() })
}

/// 次の質問 id（`q1`, `q2`, ...）を採番する。
pub fn next_question_id(run_id: &str) -> Result<String, String> {
    let n = read_raw(run_id)?
        .iter()
        .filter(|l| matches!(l, QLine::Add { .. }))
        .count();
    Ok(format!("q{}", n + 1))
}

fn read_raw(run_id: &str) -> Result<Vec<QLine>, String> {
    let path = queue_path(run_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let f = std::fs::File::open(&path).map_err(|e| format!("質問キュー読取失敗 {}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (i, line) in BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| format!("質問キュー 行 {} 読取失敗: {e}", i + 1))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let l: QLine = serde_json::from_str(line)
            .map_err(|e| format!("質問キュー 行 {} の JSON パース失敗: {e}", i + 1))?;
        out.push(l);
    }
    Ok(out)
}

/// fold 済みの質問一覧を返す。
pub fn read_questions(run_id: &str) -> Result<Vec<Question>, String> {
    let raw = read_raw(run_id)?;
    let mut out: Vec<Question> = Vec::new();
    for l in raw {
        match l {
            QLine::Add { question, .. } => out.push(Question {
                id: question.id,
                kind: question.kind,
                question: question.question,
                header: question.header,
                options: question.options,
                required: question.required,
                context_ref: question.context_ref,
                answered: false,
                answer: None,
            }),
            QLine::Answer { id, answer, .. } => {
                if let Some(q) = out.iter_mut().find(|q| q.id == id) {
                    q.answered = true;
                    q.answer = Some(answer);
                }
            }
        }
    }
    Ok(out)
}
