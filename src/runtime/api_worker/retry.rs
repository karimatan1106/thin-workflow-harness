//! 429/5xx の指数バックオフリトライ（最大 3 回）。その他 4xx は即 fail。

use crate::runtime::anthropic::{MessagesRequest, MessagesResponse};
use crate::runtime::auth::AuthMode;
use crate::runtime::http_client::{HttpClient, HttpResponse};

use super::{API_URL, API_VERSION};

/// `MessagesRequest` を POST し、429/5xx は指数バックオフでリトライする。
pub(super) fn call_with_retry(
    auth: &AuthMode,
    http: &dyn HttpClient,
    req: &MessagesRequest,
) -> Result<MessagesResponse, String> {
    let body = serde_json::to_string(req).map_err(|e| format!("リクエスト直列化失敗: {e}"))?;
    let mut headers = auth.auth_headers(API_VERSION);
    headers.push(("content-type".to_string(), "application/json".to_string()));
    let mut last_err = String::new();
    for attempt in 0..4 {
        match http.post(API_URL, &headers, &body) {
            Ok(HttpResponse { status: 200, body: text }) => {
                return serde_json::from_str::<MessagesResponse>(&text)
                    .map_err(|e| format!("レスポンスパース失敗: {e} ── body={text}"));
            }
            Ok(HttpResponse { status, body: text })
                if (500..=599).contains(&status) || status == 429 =>
            {
                last_err = format!("HTTP {status}: {text}");
                if attempt < 3 {
                    sleep_backoff(attempt);
                    continue;
                }
            }
            Ok(HttpResponse { status, body: text }) => {
                return Err(format!("HTTP {status}: {text}"));
            }
            Err(e) => {
                last_err = format!("transport: {e}");
                if attempt < 3 {
                    sleep_backoff(attempt);
                    continue;
                }
            }
        }
    }
    Err(format!("API リトライ尽きた: {last_err}"))
}

/// 200ms, 400ms, 800ms, 1600ms ── テストでもこの待ちが効くが秒オーダーには行かない。
fn sleep_backoff(attempt: usize) {
    std::thread::sleep(std::time::Duration::from_millis(200 << attempt));
}
