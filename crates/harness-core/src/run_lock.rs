//! run 単位の排他ロック ── 複数セッション/プロセスが同じ run を同時駆動するのを防ぐ。
//!
//! `harness run` は run を駆動する前に `state/<run-id>.lock` を `create_new` で排他取得する。
//! 既に存在すれば「別プロセスが駆動中」とみなして拒否する（lock ファイルに PID/ts を書くので
//! どのプロセスが握っているか分かる）。ロックは RAII で、`RunLock` が drop されると解放される
//! （正常終了・panic・早期 return いずれでも Drop が走り、ロックファイルは消える）。
//!
//! クロスプラットフォーム: OS の advisory lock (flock/LockFileEx) ではなく
//! 「ファイルの存在 = ロック」方式。Windows/Linux/macOS で同一挙動。fs の create_new は
//! atomic なので、同時 acquire のレースでも片方だけが成功する。
//!
//! stale lock（プロセスが kill されてロックファイルが残る）は、中身の PID/ts を見て
//! 人間が判断し `harness run --force-unlock`（= 事前に lock を消す）で奪取する想定。
//! 自動 PID 生存チェックは OS 依存で脆いので入れない（明示削除を促す）。

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

use chrono::{SecondsFormat, Utc};

use crate::paths;

/// run 駆動中であることを示す排他ロック。Drop で自動解放（ロックファイル削除）。
#[derive(Debug)]
pub struct RunLock {
    path: PathBuf,
}

impl RunLock {
    /// `state/<run-id>.lock` を排他取得する。既に存在すれば Err（誰が握っているか付き）。
    pub fn acquire(run_id: &str) -> Result<Self, String> {
        let path = paths::state_dir()?.join(format!("{run_id}.lock"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                // 誰が握っているか分かるよう PID と取得時刻を書く（失敗しても致命ではない）。
                let _ = writeln!(
                    f,
                    "pid={} acquired={}",
                    std::process::id(),
                    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
                );
                Ok(RunLock { path })
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let holder = std::fs::read_to_string(&path).unwrap_or_default();
                Err(format!(
                    "run '{run_id}' は別プロセスが駆動中（{}）── 同時実行は不可。\
                     stale（プロセスが既に死んでいる）なら {} を削除してから再実行せよ",
                    holder.trim(),
                    path.display()
                ))
            }
            Err(e) => Err(format!("ロック取得失敗 {}: {e}", path.display())),
        }
    }

    /// run の lock ファイルパス（force-unlock 用に公開）。
    pub fn lock_path(run_id: &str) -> Result<PathBuf, String> {
        Ok(paths::state_dir()?.join(format!("{run_id}.lock")))
    }
}

impl Drop for RunLock {
    fn drop(&mut self) {
        // 解放はベストエフォート ── 消せなくても次回 acquire が holder 情報を見せる。
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    // HARNESS_HOME はプロセスグローバルなので、env を触る test 同士を直列化する。
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    // state_dir() は HARNESS_HOME 依存なので、テストごとに一時 HOME を設定して隔離する。
    fn with_temp_home<F: FnOnce()>(f: F) {
        let _g = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let prev = std::env::var_os("HARNESS_HOME");
        std::env::set_var("HARNESS_HOME", tmp.path());
        f();
        match prev {
            Some(v) => std::env::set_var("HARNESS_HOME", v),
            None => std::env::remove_var("HARNESS_HOME"),
        }
    }

    #[test]
    fn acquire_then_second_fails_then_release_allows_reacquire() {
        with_temp_home(|| {
            let l1 = RunLock::acquire("run_x").expect("1回目は取れる");
            // 同じ run の2回目は拒否される（別プロセス相当）。
            let l2 = RunLock::acquire("run_x");
            assert!(l2.is_err(), "2回目の acquire は拒否されるべき");
            assert!(l2.unwrap_err().contains("別プロセスが駆動中"));
            // l1 を drop すると解放され、再取得できる。
            drop(l1);
            let l3 = RunLock::acquire("run_x");
            assert!(l3.is_ok(), "解放後は再取得できるべき");
        });
    }

    #[test]
    fn different_runs_do_not_conflict() {
        with_temp_home(|| {
            let _a = RunLock::acquire("run_a").expect("run_a 取得");
            let b = RunLock::acquire("run_b");
            assert!(b.is_ok(), "別 run のロックは互いに干渉しない");
        });
    }

    #[test]
    fn holder_info_contains_pid() {
        with_temp_home(|| {
            let _l = RunLock::acquire("run_pid").expect("取得");
            let p = RunLock::lock_path("run_pid").unwrap();
            let body = std::fs::read_to_string(&p).unwrap();
            assert!(body.contains("pid="), "lock ファイルに PID が記録されるべき: {body}");
        });
    }
}
