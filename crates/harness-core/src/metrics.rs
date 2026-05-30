//! ノード完了時のメトリクスサイドカー（`state/<run-id>.metrics.jsonl`、append-only）。
//!
//! 各行 1 ノード分の `{node, tool_calls, wall_seconds, ts}`（`cost` / `tokens` は本物の
//! LLM worker でのみ意味を持つので optional ── スクリプト worker では省略）。
//! イベントログを軽く保つための分離（`DESIGN.md` §16.1・`docs/operations.md` §1）。
//! メトリクスを書くのは runtime ループの責務 ── debug CLI の `advance` は書かない。

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::paths;

/// tokens の内訳（API worker 用 ── スクリプトでは None）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TokenBreakdown {
    pub input: u64,
    pub output: u64,
    #[serde(default)]
    pub cache_create: u64,
    #[serde(default)]
    pub cache_read: u64,
}

/// 1 ノード分のメトリクス行。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeMetrics {
    pub node: String,
    pub tool_calls: u64,
    pub wall_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    /// 合計トークン（input+output）── 後方互換のため u64 のまま残す。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    /// 内訳（input/output/cache_create/cache_read）── API worker のみ載せる。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_breakdown: Option<TokenBreakdown>,
    /// 使用モデル名（API worker のみ。旧 jsonl 後方互換のため default あり）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub ts: String,
}

impl NodeMetrics {
    /// スクリプト worker 用 ── cost / tokens なし。
    pub fn scripted(node: &str, tool_calls: u64, wall_seconds: f64) -> Self {
        NodeMetrics {
            node: node.to_string(),
            tool_calls,
            wall_seconds,
            cost: None,
            tokens: None,
            tokens_breakdown: None,
            model: None,
            ts: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        }
    }

    /// API worker 用 ── tokens 内訳 ＋ cost ＋ 使用モデル名。
    pub fn api(
        node: &str,
        tool_calls: u64,
        wall_seconds: f64,
        breakdown: TokenBreakdown,
        cost: Option<f64>,
        model: Option<String>,
    ) -> Self {
        let total = breakdown.input + breakdown.output;
        NodeMetrics {
            node: node.to_string(),
            tool_calls,
            wall_seconds,
            cost,
            tokens: Some(total),
            tokens_breakdown: Some(breakdown),
            model,
            ts: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_carries_model_and_round_trips() {
        // model フィールドが serialize→deserialize で保持される。
        let b = TokenBreakdown { input: 10, output: 20, cache_create: 5, cache_read: 3 };
        let m = NodeMetrics::api("design", 2, 4.0, b, Some(0.05), Some("claude-opus".into()));
        assert_eq!(m.tokens, Some(30)); // input + output のみ
        assert_eq!(m.model.as_deref(), Some("claude-opus"));
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"model\":\"claude-opus\""));
        let back: NodeMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn scripted_has_no_model_and_is_skipped() {
        // scripted は model None ── serialize 時に出力されない。
        let m = NodeMetrics::scripted("impl", 1, 1.0);
        assert!(m.model.is_none());
        let json = serde_json::to_string(&m).unwrap();
        assert!(!json.contains("model"), "json: {json}");
    }

    #[test]
    fn legacy_jsonl_without_model_deserializes() {
        // model フィールドの無い旧 jsonl 行も serde(default) で読める（後方互換）。
        let legacy = r#"{"node":"design","tool_calls":2,"wall_seconds":4.0,"cost":0.05,"tokens":30,"ts":"2026-01-01T00:00:00Z"}"#;
        let m: NodeMetrics = serde_json::from_str(legacy).unwrap();
        assert_eq!(m.node, "design");
        assert_eq!(m.tokens, Some(30));
        assert!(m.model.is_none());
    }
}

/// メトリクス行を 1 つ追記する。
pub fn append_metrics(run_id: &str, m: &NodeMetrics) -> Result<(), String> {
    let path = paths::metrics_path(run_id)?;
    let line = serde_json::to_string(m).map_err(|e| format!("metrics シリアライズ失敗: {e}"))?;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("metrics 書込失敗 {}: {e}", path.display()))?;
    writeln!(f, "{line}").map_err(|e| format!("metrics 書込失敗: {e}"))?;
    Ok(())
}

/// メトリクスサイドカーを読む。ファイル無しなら None（runtime ループ未実行）。
pub fn read_metrics(run_id: &str) -> Result<Option<Vec<NodeMetrics>>, String> {
    let path = paths::metrics_path(run_id)?;
    if !path.exists() {
        return Ok(None);
    }
    let f = std::fs::File::open(&path).map_err(|e| format!("metrics 読取失敗 {}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (i, line) in BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| format!("metrics 行 {} 読取失敗: {e}", i + 1))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let m: NodeMetrics = serde_json::from_str(line)
            .map_err(|e| format!("metrics 行 {} の JSON パース失敗: {e}", i + 1))?;
        out.push(m);
    }
    Ok(Some(out))
}
