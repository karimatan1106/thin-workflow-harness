//! ApiWorker 経路の fork/join 並列駆動 ── scripted 版 `fork_join::run_parallel_scripted`
//! の API 版。各 branch を `std::thread::scope` 内で並列起動し、各 thread が
//! `api_runner::branch::drive_branch_api` を呼ぶ。
//!
//! 役割分担:
//! - 事前 `blast_radius_disjoint` チェック（既存 `fork_join::check_pairwise_disjoint` を再利用）。
//! - メイン log への `BranchForked` append（並列開始マーカー）。
//! - 各 branch を `thread::scope` で並列駆動 ── `HttpClient: Send+Sync` 制約のもと、
//!   `&dyn HttpClient` / `&Workflow` を thread に borrow させる。`'static` clone 不要。
//! - `ScopedJoinHandle::join()` の `thread::Result<Result<(), String>>` を集約 ──
//!   Ok(Ok) = 成功、Ok(Err) = branch 失敗、Err = panic（いずれも failures に積まれる）。
//! - 全成功なら `BranchJoined{status:"success", failures:None}` を append、Ok(())。
//! - 1 つでも失敗なら `BranchJoined{status:"failed", failures:Some(...)}` を append、
//!   集約メッセージで Err。
//!
//! ## なぜ thread::scope か
//! `&dyn HttpClient` は `Send+Sync` だが `'static` ではない。`std::thread::spawn` に渡すには
//! `'static` clone（or `Arc<dyn HttpClient>`）が要る。`thread::scope` ならスコープ内で
//! borrow が成立するので、`RunnerDeps` の `&'a dyn HttpClient` を Arc 化せずそのまま渡せる。
//! Workflow も同様 ── owned clone も Arc も不要、`&Workflow` だけで足りる。
//!
//! ## dispatch
//! `api_runner::run_loop` から `node.node_type() == "fork"` のとき本関数に振る。
//! 成功時の Advance event 書き込みは `run_loop` 側で行う（scripted 経路と対称）。

use std::thread;

use crate::event::{append_event, EventKind};
use crate::runtime::api_runner::branch::drive_branch_api;
use crate::runtime::auth::AuthMode;
use crate::runtime::fork_join::check_pairwise_disjoint;
use crate::runtime::http_client::HttpClient;
use crate::workflow::{Node, Workflow};

/// ApiWorker 経路で fork ノードの全 branch を並列駆動する。
///
/// 戻り値:
/// - 全 branch 成功 → `Ok(())`、メイン log に `BranchForked` + `BranchJoined{success}`。
/// - 1 つでも失敗 → `Err`、メイン log に `BranchForked` + `BranchJoined{failed, failures}`。
/// - blast_radius_disjoint 違反 → `Err` 即返し、log には何も書かない（scripted 版と同じ）。
pub fn run_parallel_api(
    run_id: &str,
    workflow: &Workflow,
    fork_node: &Node,
    http: &dyn HttpClient,
    auth: &AuthMode,
    model_default: &str,
    cwd: &std::path::Path,
) -> Result<(), String> {
    let branches: Vec<String> = fork_node.branches.clone();
    if branches.len() < 2 {
        return Err(format!(
            "fork node {} has fewer than 2 branches; cannot parallelize",
            fork_node.id
        ));
    }
    check_pairwise_disjoint(run_id, workflow, &branches)?;

    append_event(run_id, EventKind::BranchForked { branch_ids: branches.clone() })
        .map_err(|e| format!("branch_forked write fail: {e}"))?;
    println!("[fork {}] spawning api branches={:?}", fork_node.id, branches);

    let failures = run_branches_scoped(run_id, &branches, workflow, http, auth, model_default, cwd)?;

    if !failures.is_empty() {
        append_event(
            run_id,
            EventKind::BranchJoined {
                branch_ids: branches.clone(),
                status: "failed".into(),
                failures: Some(failures.clone()),
            },
        )
        .map_err(|e| format!("branch_joined write fail: {e}"))?;
        return Err(format!(
            "fork {}: {} branch(es) failed: {}",
            fork_node.id,
            failures.len(),
            failures.join(" / ")
        ));
    }
    append_event(
        run_id,
        EventKind::BranchJoined {
            branch_ids: branches.clone(),
            status: "success".into(),
            failures: None,
        },
    )
    .map_err(|e| format!("branch_joined write fail: {e}"))?;
    println!("[fork {}] all api branches ok", fork_node.id);
    Ok(())
}

/// scope 内で全 branch を並列駆動し、failures を集約して返す。
/// spawn 失敗（OS リソース不足等、稀）は即 Err、それ以外は thread join 結果を畳む。
fn run_branches_scoped(
    run_id: &str,
    branches: &[String],
    workflow: &Workflow,
    http: &dyn HttpClient,
    auth: &AuthMode,
    model_default: &str,
    cwd: &std::path::Path,
) -> Result<Vec<String>, String> {
    let mut spawn_err: Option<String> = None;
    let failures = thread::scope(|s| {
        let mut handles: Vec<(String, thread::ScopedJoinHandle<'_, Result<(), String>>)> =
            Vec::with_capacity(branches.len());
        for bid in branches {
            let bid_clone = bid.clone();
            let bid_thread_name = bid.clone();
            let h = thread::Builder::new()
                .name(format!("api-branch-{bid_thread_name}"))
                .spawn_scoped(s, move || {
                    drive_branch_api(
                        run_id,
                        &bid_clone,
                        workflow,
                        http,
                        auth,
                        model_default,
                        cwd,
                    )
                });
            match h {
                Ok(handle) => handles.push((bid.clone(), handle)),
                Err(e) => {
                    spawn_err = Some(format!("api branch thread spawn fail ({bid}): {e}"));
                    break;
                }
            }
        }
        join_all_scoped(handles)
    });
    if let Some(e) = spawn_err {
        return Err(e);
    }
    Ok(failures)
}

/// 全 thread の戻り値を集約。失敗（branch Err / panic）を `failures` Vec で返す。
fn join_all_scoped(
    handles: Vec<(String, thread::ScopedJoinHandle<'_, Result<(), String>>)>,
) -> Vec<String> {
    let mut failures: Vec<String> = Vec::new();
    for (bid, h) in handles {
        match h.join() {
            Ok(Ok(())) => println!("[api branch {bid}] done"),
            Ok(Err(e)) => {
                println!("[api branch {bid}] fail: {e}");
                failures.push(format!("{bid}: {e}"));
            }
            Err(_) => {
                println!("[api branch {bid}] panic");
                failures.push(format!("{bid}: panic"));
            }
        }
    }
    failures
}
