//! ApiWorker 経路の fork/join 並列駆動 ── scripted 版 `fork_join::run_parallel_scripted`
//! の API 版。各 branch を std::thread::spawn で並列起動し、各 thread が
//! `api_runner::branch::drive_branch_api` を呼ぶ（前バッチ d5cad72 の足場）。
//!
//! 役割分担:
//! - 事前 `blast_radius_disjoint` チェック（既存 `fork_join::check_pairwise_disjoint` を再利用）。
//! - メイン log への `BranchForked` append（並列開始マーカー）。
//! - `Arc<dyn HttpClient>` を全 thread で共有（HttpClient: Send+Sync 制約に依存）。
//! - `Arc<Workflow>` / `AuthMode::clone` / cwd・model_default の owned clone で `static`
//!   thread に渡す（参照を thread に持ち込まない）。
//! - `JoinHandle::join()` の thread::Result<Result<(), String>> を集約 ── Ok(Ok) =
//!   成功、Ok(Err) = branch 失敗、Err = panic（いずれも failures に積まれる）。
//! - 全成功なら `BranchJoined{status:"success", failures:None}` を append、Ok(())。
//! - 1 つでも失敗なら `BranchJoined{status:"failed", failures:Some(...)}` を append、
//!   集約メッセージで Err。
//!
//! ## なぜ Arc<Workflow>
//! drive_branch_api は `&Workflow` を取るが、thread closure に持ち込むには `static`
//! が要る。owned clone は重くなる + Node 探索が複数 branch で重複するので Arc で共有
//! する（read-only）。AuthMode は Clone なので各 thread 個別に持つ（軽量）。
//!
//! ## dispatch は未着手
//! `api_runner::run_loop` から type=fork を本関数に振る配線は次バッチ。本ファイルは
//! 「呼べる土台」までを提供する。

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use crate::event::{append_event, EventKind};
use crate::runtime::api_runner::branch::drive_branch_api;
use crate::runtime::auth::AuthMode;
use crate::runtime::http_client::HttpClient;
use crate::runtime::fork_join::check_pairwise_disjoint;
use crate::workflow::{Node, Workflow};

/// 各 branch の thread join handle と branch_id のペア。
type BranchHandle = (String, thread::JoinHandle<Result<(), String>>);

/// ApiWorker 経路で fork ノードの全 branch を並列駆動する。
///
/// 戻り値:
/// - 全 branch 成功 → `Ok(())`、メイン log に `BranchForked` + `BranchJoined{success}`。
/// - 1 つでも失敗 → `Err`、メイン log に `BranchForked` + `BranchJoined{failed, failures}`。
/// - blast_radius_disjoint 違反 → `Err` 即返し、log には何も書かない（scripted 版と同じ）。
pub fn run_parallel_api(
    run_id: &str,
    workflow: Arc<Workflow>,
    fork_node: &Node,
    http: Arc<dyn HttpClient>,
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
    check_pairwise_disjoint(run_id, &workflow, &branches)?;

    append_event(run_id, EventKind::BranchForked { branch_ids: branches.clone() })
        .map_err(|e| format!("branch_forked write fail: {e}"))?;
    println!("[fork {}] spawning api branches={:?}", fork_node.id, branches);

    let handles = spawn_branches(
        run_id,
        &branches,
        Arc::clone(&workflow),
        Arc::clone(&http),
        auth,
        model_default,
        cwd,
    )?;

    let failures = join_all(handles);

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

/// 各 branch ごとに thread を起動する。スピンナップ失敗 1 件で全体 Err（spawn 失敗は稀）。
fn spawn_branches(
    run_id: &str,
    branches: &[String],
    workflow: Arc<Workflow>,
    http: Arc<dyn HttpClient>,
    auth: &AuthMode,
    model_default: &str,
    cwd: &std::path::Path,
) -> Result<Vec<BranchHandle>, String> {
    let mut handles = Vec::with_capacity(branches.len());
    for bid in branches {
        let bid_owned = bid.clone();
        let run_owned = run_id.to_string();
        let wf_arc = Arc::clone(&workflow);
        let http_arc = Arc::clone(&http);
        let auth_clone = auth.clone();
        let model_owned = model_default.to_string();
        let cwd_owned: PathBuf = cwd.to_path_buf();
        let h = thread::Builder::new()
            .name(format!("api-branch-{bid_owned}"))
            .spawn(move || {
                drive_branch_api(
                    &run_owned,
                    &bid_owned,
                    &wf_arc,
                    &*http_arc,
                    &auth_clone,
                    &model_owned,
                    &cwd_owned,
                )
            })
            .map_err(|e| format!("api branch thread spawn fail ({bid}): {e}"))?;
        handles.push((bid.clone(), h));
    }
    Ok(handles)
}

/// 全 thread の戻り値を集約。失敗（branch Err / panic）を `failures` Vec で返す。
fn join_all(handles: Vec<BranchHandle>) -> Vec<String> {
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
