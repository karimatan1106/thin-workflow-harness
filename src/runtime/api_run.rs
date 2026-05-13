//! `harness run [--model M]` ── CLI ハンドラ層。
//!
//! ctx 構築・auth resolve・worker spawn のオーケストレーション responsibility を持つ。
//! ノードループ本体は `api_runner.rs` に分離。
//!
//! `runtime/mod.rs` の `cmd_run`（ScriptedWorker）と並行する関数。両者の差はワーカー実装と
//! apply ループの形だけ。

use crate::runtime::api_runner::{run_loop, RunnerDeps};

/// `harness run` の既定 spawn 上限。連続 BudgetExceeded early-fail（max 2）で
/// 先に止まる想定で、暴走時のみ最終防壁としてこの値で停止する。
/// 旧 256 から縮めて「実装ミスや無限 advance_rejected ループ」を 1 桁早く検知。
pub const DEFAULT_MAX_SPAWNS: usize = 10;

use crate::runtime::auth::{resolve_auth, AuthMode};
use crate::runtime::http_client::{HttpClient, UreqClient};
use crate::workflow::Workflow;

use crate::handlers::load_wf;
use crate::paths;

/// `--script` 無しで `harness run` が呼ばれたときのエントリポイント。
pub fn cmd_run_api(
    run: Option<&str>,
    worktree: Option<&str>,
    model_override: Option<&str>,
) -> Result<(), String> {
    let http = UreqClient::default();
    cmd_run_api_with(run, worktree, model_override, &http)
}

/// `cmd_run_api` の HTTP クライアント注入版 ── テストで `MockClient` を渡すために使う。
pub fn cmd_run_api_with(
    run: Option<&str>,
    worktree: Option<&str>,
    model_override: Option<&str>,
    http: &dyn HttpClient,
) -> Result<(), String> {
    let auth = resolve_auth()?;
    cmd_run_api_with_auth(run, worktree, model_override, http, auth)
}

/// 認証モードも注入する版 ── テストで `AuthMode` を直に渡す。
pub fn cmd_run_api_with_auth(
    run: Option<&str>,
    worktree: Option<&str>,
    model_override: Option<&str>,
    http: &dyn HttpClient,
    auth: AuthMode,
) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let wf = load_wf()?;
    let cwd = worktree
        .map(std::path::PathBuf::from)
        .unwrap_or_else(paths::harness_home);
    let model_default = default_model(&wf, model_override);
    println!(
        "[runtime] run {run_id} を ApiWorker で駆動 cwd={} model_default={model_default}",
        cwd.display()
    );

    let deps = RunnerDeps {
        run_id,
        wf,
        cwd,
        model_default,
        http,
        auth,
        max_spawns: DEFAULT_MAX_SPAWNS,
    };
    run_loop(deps)
}

fn default_model(wf: &Workflow, override_: Option<&str>) -> String {
    if let Some(m) = override_ {
        return m.to_string();
    }
    wf.meta
        .default_model
        .clone()
        .unwrap_or_else(|| "claude-opus-4-7".to_string())
}


#[cfg(test)]
mod tests {
    use super::*;

    /// 既定 spawn 上限が暴走防止のために小さい値（旧 256 から大幅縮小）であることを保証する。
    /// 16 以上に膨らんだら早期検知の意味が薄れる、4 未満だと正常な再 spawn で到達してしまう。
    /// 回帰テストとして範囲を固定（const 比較なので clippy `assertions_on_constants` を許容）。
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn default_max_spawns_is_small_enough_for_runaway_detection() {
        const N: usize = DEFAULT_MAX_SPAWNS;
        assert!(N <= 16, "DEFAULT_MAX_SPAWNS が緩すぎる: {N}");
        assert!(N >= 4, "DEFAULT_MAX_SPAWNS が小さすぎる: {N}");
    }
}
