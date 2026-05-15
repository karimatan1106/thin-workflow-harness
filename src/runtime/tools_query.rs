//! `query_*` 系 tool の入力正規化と subprocess 起動。
//!
//! ToolCall::Query が保持する `QuerySpec` 定義と、`tool_use_to_call` 用ビルダ、
//! および `apply_dispatch` から呼ばれる subprocess 実行関数 `run_query` を含む。
//! subprocess は `HARNESS_BIN` 環境変数があればそれ、無ければ PATH の `harness` を使う
//! （test では `CARGO_BIN_EXE_harness` を `HARNESS_BIN` に渡して隔離する）。

use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// `harness query <subcommand> [args...]` の正規化された引数バンドル。
/// `run_query` が `std::process::Command` に直訳する。
#[derive(Debug, Clone)]
pub struct QuerySpec {
    /// `harness query` 直下のサブコマンド名（"outline" / "symbol" / "refs" / "callers" /
    /// "closure" / "impacted-by" / "tested-by"）。
    pub subcommand: String,
    /// positional arg（file or qname）── 1 個固定。
    pub positional: String,
    /// `--depth N`。Some なら付ける。
    pub depth: Option<u32>,
    /// `--direction in|out|both`（closure 用）。Some なら付ける。
    pub direction: Option<String>,
    /// `--root <path>`。Some なら付ける（無ければ subprocess cwd で相対解決）。
    pub root: Option<String>,
    /// `--format text|json`。Some なら付ける（既定 text）。
    pub format: Option<String>,
}

/// query_* 系（outline 以外）の共通ビルダ ── qname/depth/direction/root/format を拾う。
pub fn build_query_spec(tool_name: &str, input: &Value) -> Result<QuerySpec, String> {
    let qname = input.get("qname").and_then(|v| v.as_str())
        .ok_or_else(|| format!("tool '{tool_name}' に必須キー 'qname'（string）が無い"))?
        .to_string();
    // tool 名 query_impacted_by → "impacted-by"、query_symbol → "symbol"。
    let sub = tool_name.strip_prefix("query_").unwrap_or(tool_name).replace('_', "-");
    let depth = input.get("depth").and_then(|v| v.as_u64()).map(|n| n as u32);
    let direction = input.get("direction").and_then(|v| v.as_str()).map(|s| s.to_string());
    let root = input.get("root").and_then(|v| v.as_str()).map(|s| s.to_string());
    let format = input.get("format").and_then(|v| v.as_str()).map(|s| s.to_string());
    Ok(QuerySpec { subcommand: sub, positional: qname, depth, direction, root, format })
}

/// `harness` バイナリのパスを決める。`HARNESS_BIN` 環境変数が立っていればそれ、
/// 無ければ PATH 経由の `harness` を返す（cargo install 済みを前提）。
pub fn harness_bin() -> String {
    std::env::var("HARNESS_BIN").unwrap_or_else(|_| "harness".to_string())
}

/// `QuerySpec` を実行する。stdout を返す（is_error は呼び出し側で status を見て判定）。
/// cwd は呼び出し側の Interceptor が握っている値を渡す ── ApiWorker の作業ディレクトリ。
pub struct QueryOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// `harness query <sub> <positional> [--depth N] [--direction D] [--root R] [--format F]` を実行。
pub fn run_query(spec: &QuerySpec, cwd: &Path) -> Result<QueryOutput, String> {
    let bin = harness_bin();
    let mut cmd = Command::new(&bin);
    cmd.arg("query").arg(&spec.subcommand).arg(&spec.positional).current_dir(cwd);
    if let Some(d) = spec.depth { cmd.arg("--depth").arg(d.to_string()); }
    if let Some(dir) = &spec.direction { cmd.arg("--direction").arg(dir); }
    if let Some(r) = &spec.root { cmd.arg("--root").arg(r); }
    if let Some(f) = &spec.format { cmd.arg("--format").arg(f); }
    let out = cmd.output().map_err(|e| format!("subprocess '{bin}' 起動失敗: {e}"))?;
    Ok(QueryOutput {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        success: out.status.success(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_query_spec_minimal_qname_only() {
        let q = build_query_spec("query_refs", &json!({"qname":"foo::bar"})).unwrap();
        assert_eq!(q.subcommand, "refs");
        assert_eq!(q.positional, "foo::bar");
        assert!(q.depth.is_none() && q.direction.is_none() && q.root.is_none() && q.format.is_none());
    }

    #[test]
    fn build_query_spec_full_args() {
        let q = build_query_spec("query_closure",
            &json!({"qname":"x", "depth":5, "direction":"both", "root":"/r", "format":"json"})).unwrap();
        assert_eq!(q.depth, Some(5));
        assert_eq!(q.direction.as_deref(), Some("both"));
        assert_eq!(q.root.as_deref(), Some("/r"));
        assert_eq!(q.format.as_deref(), Some("json"));
    }

    #[test]
    fn build_query_spec_missing_qname_errors() {
        assert!(build_query_spec("query_callers", &json!({})).is_err());
    }

    #[test]
    fn harness_bin_respects_env_override() {
        std::env::set_var("HARNESS_BIN", "C:/tmp/fake-harness.exe");
        assert_eq!(harness_bin(), "C:/tmp/fake-harness.exe");
        std::env::remove_var("HARNESS_BIN");
        assert_eq!(harness_bin(), "harness");
    }
}
