//! HTTP クライアント抽象 ── 本番 `UreqClient`（同期、依存最小）／テスト `MockClient`。
//!
//! `docs/implementation.md` で「ureq 既定」。`ApiWorker` は `Box<dyn HttpClient>` を持つので
//! テストでは実 API を叩かずに `MockClient` を注入できる ── スクリプト化されたレスポンス列を
//! 順に返し、API レート制限・コスト・非決定性を持ち込まない。

use std::sync::Mutex;
use std::time::Duration;

/// HTTP レスポンスの最小抽象。
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// ステータスコード（200 / 4xx / 5xx）。
    pub status: u16,
    /// レスポンスボディ（文字列）。
    pub body: String,
}

/// 本番／テストで差し替えるための trait（`ApiWorker` は `Box<dyn HttpClient>` を持つ）。
pub trait HttpClient: Send + Sync {
    /// POST `url`。`headers` は `(name, value)` の列、`body` は文字列（JSON 想定）。
    fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<HttpResponse, String>;
}

/// 本番用 ── `ureq` の同期クライアント（thin、async 不要）。
pub struct UreqClient {
    agent: ureq::Agent,
}

impl Default for UreqClient {
    fn default() -> Self {
        UreqClient::new(Duration::from_secs(120))
    }
}

impl UreqClient {
    /// timeout 付きで構築。
    pub fn new(timeout: Duration) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(timeout)
            .build();
        UreqClient { agent }
    }
}

impl HttpClient for UreqClient {
    fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<HttpResponse, String> {
        let mut req = self.agent.post(url);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        match req.send_string(body) {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.into_string().unwrap_or_default();
                Ok(HttpResponse { status, body: text })
            }
            // ureq は 4xx/5xx を Err にする ── ステータスを取り出してそのまま返す。
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Ok(HttpResponse { status: code, body: text })
            }
            Err(e) => Err(format!("HTTP transport error: {e}")),
        }
    }
}

/// テスト用 ── スクリプト化されたレスポンス列を 1 回呼ばれるごとに 1 つ消費する。
///
/// レスポンスが「成功」「失敗（ネットワーク or 5xx）」両方をシミュレートできるよう、
/// 各エントリは `Result<HttpResponse, String>`。`status=429/500` を返すか、
/// `Err("network down")` を返すかでリトライ経路をテストする。
pub struct MockClient {
    scripted: Mutex<Vec<Result<HttpResponse, String>>>,
    calls: Mutex<Vec<MockCall>>,
}

/// 1 回の POST 呼び出しの記録（テスト assertion 用）── headers も含めて記録する
/// （prompt-caching beta header / cache_control の検証のため）。
#[derive(Debug, Clone)]
pub struct MockCall {
    pub url: String,
    pub body: String,
    pub headers: Vec<(String, String)>,
}

impl MockClient {
    /// レスポンス列を渡して構築。`responses[0]` から順に消費する。
    pub fn new(responses: Vec<Result<HttpResponse, String>>) -> Self {
        MockClient {
            scripted: Mutex::new(responses),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// 残りキューを返す（テストで「全部消費した」を検査するため）。
    pub fn remaining(&self) -> usize {
        self.scripted.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// 記録された呼び出し列を返す。
    pub fn calls(&self) -> Vec<MockCall> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }
}

impl HttpClient for MockClient {
    fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<HttpResponse, String> {
        if let Ok(mut c) = self.calls.lock() {
            c.push(MockCall {
                url: url.to_string(),
                body: body.to_string(),
                headers: headers.to_vec(),
            });
        }
        let mut q = self.scripted.lock().map_err(|e| format!("mock lock poisoned: {e}"))?;
        if q.is_empty() {
            return Err("MockClient: スクリプト切れ（想定外の呼び出し）".to_string());
        }
        q.remove(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_client_consumes_scripted_responses_in_order() {
        let mc = MockClient::new(vec![
            Ok(HttpResponse { status: 200, body: "first".into() }),
            Ok(HttpResponse { status: 429, body: "rate".into() }),
        ]);
        let r1 = mc.post("u", &[], "b1").unwrap();
        assert_eq!(r1.body, "first");
        let r2 = mc.post("u", &[], "b2").unwrap();
        assert_eq!(r2.status, 429);
        assert_eq!(mc.remaining(), 0);
        assert_eq!(mc.calls().len(), 2);
    }

    #[test]
    fn mock_client_errors_when_exhausted() {
        let mc = MockClient::new(vec![]);
        assert!(mc.post("u", &[], "b").is_err());
    }

    #[test]
    fn mock_client_propagates_transport_error() {
        let mc = MockClient::new(vec![Err("network down".into())]);
        let r = mc.post("u", &[], "b");
        assert_eq!(r.unwrap_err(), "network down");
    }
}
