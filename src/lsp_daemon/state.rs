//! daemon の runtime state ── 起動時刻 / 累計 query 数 / 直近 wall time / 直近 request 時刻。
//!
//! health check API (`Op::Health`) でこの snapshot を返す。LLM agent が
//! 「daemon が indexing 中か / 安定 hot 状態か」を data 駆動で判断する。
//!
//! - `started_at`: daemon 起動時刻 (Instant)
//! - `queries_handled`: 累計 query 数 (AtomicU64、health op は除外)
//! - `recent`: 直近 N=10 件の wall time (ms)、push 時 FIFO trim
//! - `last_request_at`: 直近 request 着信時刻 (Instant)。idle watcher 用。
//!
//! Scope: foreground daemon 1 lang per process / record_query は handle_request
//! 終端で呼ばれる / snapshot は lock-light で OK。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::ckg::lsp::Lang;

const RECENT_MAX: usize = 10;

pub struct DaemonState {
    pub lang: Lang,
    pub started_at: Instant,
    pub queries_handled: AtomicU64,
    pub recent: Mutex<Vec<u64>>,
    /// 直近 request 着信時刻。idle watcher が `idle_duration` で参照する。
    pub last_request_at: Mutex<Instant>,
}

impl DaemonState {
    pub fn new(lang: Lang) -> Self {
        let now = Instant::now();
        Self {
            lang,
            started_at: now,
            queries_handled: AtomicU64::new(0),
            recent: Mutex::new(Vec::with_capacity(RECENT_MAX)),
            last_request_at: Mutex::new(now),
        }
    }

    /// 1 request 着信時に呼ぶ。idle timer を reset する。
    pub fn touch(&self) {
        let mut g = match self.last_request_at.lock() {
            Ok(g) => g,
            Err(poison) => poison.into_inner(),
        };
        *g = Instant::now();
    }

    /// 直近 request からの経過時間。idle watcher が threshold と比較する。
    pub fn idle_duration(&self) -> Duration {
        let g = match self.last_request_at.lock() {
            Ok(g) => g,
            Err(poison) => poison.into_inner(),
        };
        g.elapsed()
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

    #[test]
    fn touch_resets_idle_duration() {
        let s = DaemonState::new(Lang::Rust);
        std::thread::sleep(Duration::from_millis(40));
        let before = s.idle_duration();
        assert!(before >= Duration::from_millis(30), "before too small: {:?}", before);
        s.touch();
        let after = s.idle_duration();
        assert!(after < before, "after ({:?}) should be < before ({:?})", after, before);
        assert!(after < Duration::from_millis(20), "after too large: {:?}", after);
    }

    #[test]
    fn idle_duration_grows_without_touch() {
        let s = DaemonState::new(Lang::Go);
        let d1 = s.idle_duration();
        std::thread::sleep(Duration::from_millis(30));
        let d2 = s.idle_duration();
        assert!(d2 > d1, "d2 ({:?}) should grow vs d1 ({:?})", d2, d1);
    }
}
