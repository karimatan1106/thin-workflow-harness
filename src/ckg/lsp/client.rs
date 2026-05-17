//! LSP 同期クライアント ── 1 subprocess に対して req/resp を 1 往復ずつ流す。
//!
//! - subprocess を spawn（stdio パイプ）
//! - initialize → initialized 通知 → 任意のクエリ → shutdown → exit
//! - 通知（method ありで id 無し）と response（id あり）を peek 区別、resp に該当する id だけ返す
//! - rust-analyzer の progress / log 系通知は黙って捨てる
//! - Windows 専用 ── `.cmd` shim 解決のため `cmd /c <cmd>` フォールバックを持つ

use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStdin, ChildStdout};

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

use super::client_spawn::{spawn_child, spawn_via_cmd};
use super::framing::{read_message, write_message};
use super::init_options::init_options;
use super::lang::Lang;

/// spawned な LSP プロセスを保持する同期クライアント。
pub struct LspClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
    /// `start_for_lang` 経由のみ Some。legacy `spawn` 経路は None で init_options を送らない。
    lang: Option<Lang>,
}

impl LspClient {
    /// LSP サーバを spawn する（PATH 経由、args 無し）。Rust の rust-analyzer 等向け。
    pub fn spawn(cmd: &str) -> Result<Self, String> {
        Self::spawn_with_args(cmd, &[])
    }

    /// LSP サーバを spawn する。args は typescript-language-server の `--stdio` 等で使う。
    ///
    /// Windows では npm-global の `typescript-language-server` が `.cmd` shim として
    /// 置かれることが多く、`Command::new` は PATHEXT 解決をしないため直 spawn が失敗する。
    /// 失敗時に `cmd /c <cmd> <args...>` で起動する fallback を持つ。
    pub fn spawn_with_args(cmd: &str, args: &[String]) -> Result<Self, String> {
        let mut child = match spawn_child(cmd, args) {
            Ok(c) => c,
            Err(direct_err) => match spawn_via_cmd(cmd, args) {
                Ok(c) => c,
                Err(e) => {
                    return Err(format!(
                        "spawn {cmd}: direct={direct_err}; via cmd: {e}"
                    ));
                }
            },
        };
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "no stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "no stdout".to_string())?;
        Ok(LspClient {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
            lang: None,
        })
    }

    /// `Lang` から server コマンドを解決して spawn する薄いラッパ。
    pub fn start_for_lang(lang: super::lang::Lang) -> Result<Self, String> {
        let (cmd, args) = super::lang::lsp_server_cmd(lang);
        let mut c = Self::spawn_with_args(&cmd, &args)?;
        c.lang = Some(lang);
        Ok(c)
    }

    /// `initialize` リクエスト + `initialized` 通知。`root_uri` は file:// URI。
    /// `start_for_lang` 経由なら Lang 別 init_options（TS は tsserver preferences）を同梱。
    pub fn initialize(&mut self, root_uri: &str) -> Result<Value, String> {
        // capabilities は最小限。workspace symbol を確実に有効化。
        let mut params = json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "workspace": {
                    "symbol": { "dynamicRegistration": false }
                }
            },
            "workspaceFolders": [
                { "uri": root_uri, "name": "root" }
            ],
        });
        let opts = self.lang.map(init_options).unwrap_or(Value::Null);
        if !opts.is_null() {
            params["initializationOptions"] = opts;
        }
        let result = self.request("initialize", params)?;
        self.notify("initialized", json!({}))?;
        Ok(result)
    }

    /// `shutdown` → `exit` → child を待つ。
    pub fn shutdown(mut self) -> Result<(), String> {
        let _: Result<Value, _> = self.request("shutdown", Value::Null);
        let _ = self.notify("exit", Value::Null);
        let _ = self.child.wait();
        Ok(())
    }

    /// 任意の JSON-RPC リクエスト ── 通知は捨ててマッチした response の `result` を返す。
    pub fn request<R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: impl Serialize,
    ) -> Result<R, String> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let body = serde_json::to_string(&msg).map_err(|e| format!("serialize: {e}"))?;
        write_message(&mut self.stdin, &body)?;
        self.wait_response(id)
    }

    /// 通知（id なし） ── response を期待しない。
    pub fn notify(&mut self, method: &str, params: impl Serialize) -> Result<(), String> {
        let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        let body = serde_json::to_string(&msg).map_err(|e| format!("serialize: {e}"))?;
        write_message(&mut self.stdin, &body)
    }

    fn wait_response<R: DeserializeOwned>(&mut self, id: i64) -> Result<R, String> {
        loop {
            let msg = read_one(&mut self.stdout)?;
            let v: Value = serde_json::from_str(&msg).map_err(|e| format!("parse: {e}"))?;
            // 通知（method あり, id なし） → 捨てる
            if v.get("method").is_some() && v.get("id").is_none() {
                continue;
            }
            // server→client request（method + id）は ack で握り潰す（rust-analyzer の progress 等）
            if v.get("method").is_some() && v.get("id").is_some() {
                let sid = v.get("id").cloned().unwrap_or(Value::Null);
                let ack = json!({ "jsonrpc": "2.0", "id": sid, "result": null });
                let body = serde_json::to_string(&ack).map_err(|e| format!("ack serialize: {e}"))?;
                write_message(&mut self.stdin, &body)?;
                continue;
            }
            // response（id あり）
            if let Some(rid) = v.get("id").and_then(|x| x.as_i64()) {
                if rid != id {
                    continue;
                }
                if let Some(err) = v.get("error") {
                    return Err(format!("lsp error: {err}"));
                }
                let result = v.get("result").cloned().unwrap_or(Value::Null);
                return serde_json::from_value::<R>(result)
                    .map_err(|e| format!("deserialize result: {e}"))
            }
        }
    }
}

/// LSP client を spawn -> initialize -> (TS なら) didOpen まで一通り走らせて
/// 「query を投げ込める状態」にして返す。複数 query を 1 client で連投する
/// hot path (layer 2.5 PoC) で使う。
pub fn start_and_warm_up(
    lang: super::lang::Lang,
    root: &std::path::Path,
) -> Result<LspClient, String> {
    let mut client = LspClient::start_for_lang(lang)?;
    let root_uri = super::query::path_to_file_uri(root)?;
    let _ = client.initialize(&root_uri)?;
    if matches!(lang, super::lang::Lang::Ts) {
        super::ts_bootstrap::warm_up_ts_workspace(&mut client, root)?;
    }
    Ok(client)
}

fn read_one<R: BufRead>(r: &mut R) -> Result<String, String> {
    match read_message(r)? {
        Some(m) => Ok(m),
        None => Err("server closed stdout unexpectedly".to_string()),
    }
}
