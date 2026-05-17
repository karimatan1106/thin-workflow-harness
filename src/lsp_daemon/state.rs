//! daemon の runtime state ── 起動時刻 / 累計 query 数 / 直近 wall time。
//!
//! health check API (`Op::Health`) でこの snapshot を返す。LLM agent が
//! 「daemon が indexing 中か / 安定 hot 状態か」を data 駆動で判断する。
//!
//! - `started_at`: daemon 起動時刻 (Instant)
//! - `queries_handled`: 累計 query 数 (AtomicU64、health op は除外)
//! - `recent`: 直近 N=10 件の wall time (ms)、push 時 FIFO trim
//!
//! Scope: foreground daemon 1 lang per process / record_query は handle_request
//! 終端で呼ばれる / snapshot は lock-light で OK。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::ckg::lsp::Lang;

const RECENT_MAX: usize = 10;

pub struct DaemonState {
    pub lang: Lang,
    pub started_at: Instant,
    pub queries_handled: AtomicU64,
    pub recent: Mutex<Vec<u64>>,
}

impl DaemonState {
    pub fn new(lang: Lang) -> Self {
        Self {
            lang,
            started_at: Instant::now(),
            queries_handled: AtomicU64::new(0),
            recent: Mutex::new(Vec::with_capacity(RECENT_MAX)),
        }
    }

    /// 1 query 終了時に呼ぶ。累計 + 直近 buffer を更新。
    pub fn record_query(&self, elapsed_ms: u64) {
        self.queries_handled.fetch_add(1, Ordering::Relaxed);
        let mut v = match self.recent.lock() {
            Ok(g) => g,
            Err(poison) => poison.into_inner(),
        };
        if v.len() >= RECENT_MAX {
            v.remove(0);
        }
        v.push(elapsed_ms);
    }

    /// health response 用の snapshot。recent_avg は直近 buffer の整数平均。
    pub fn snapshot(&self) -> StateSnapshot {
        let recent: Vec<u64> = {
            let g = match self.recent.lock() {
                Ok(g) => g,
                Err(poison) => poison.into_inner(),
            };
            g.clone()
        };
        let recent_avg_ms = if recent.is_empty() {
            0
        } else {
            recent.iter().sum::<u64>() / recent.len() as u64
        };
        StateSnapshot {
            lang: lang_to_str(self.lang).to_string(),
            uptime_ms: self.started_at.elapsed().as_millis() as u64,
            queries_handled: self.queries_handled.load(Ordering::Relaxed),
            recent_avg_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub lang: String,
    pub uptime_ms: u64,
    pub queries_handled: u64,
    pub recent_avg_ms: u64,
}

fn lang_to_str(l: Lang) -> &'static str {
    match l {
        Lang::Rust => "rust",
        Lang::Ts => "ts",
        Lang::Py => "py",
        Lang::Go => "go",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_empty_recent_returns_zero_avg() {
        let s = DaemonState::new(Lang::Rust);
        let snap = s.snapshot();
        assert_eq!(snap.lang, "rust");
        assert_eq!(snap.queries_handled, 0);
        assert_eq!(snap.recent_avg_ms, 0);
    }

    #[test]
    fn record_query_increments_and_averages() {
        let s = DaemonState::new(Lang::Ts);
        s.record_query(100);
        s.record_query(200);
        s.record_query(300);
        let snap = s.snapshot();
        assert_eq!(snap.lang, "ts");
        assert_eq!(snap.queries_handled, 3);
        assert_eq!(snap.recent_avg_ms, 200);
    }

    #[test]
    fn recent_buffer_caps_at_ten() {
        let s = DaemonState::new(Lang::Py);
        for ms in 0..15u64 {
            s.record_query(ms * 10);
        }
        let snap = s.snapshot();
        assert_eq!(snap.queries_handled, 15);
        // 直近 10 件 = [50,60,...,140] → 平均 95
        assert_eq!(snap.recent_avg_ms, 95);
    }
}
