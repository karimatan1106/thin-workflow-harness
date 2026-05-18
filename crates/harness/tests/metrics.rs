//! metrics サイドカー（`state/<run-id>.metrics.jsonl`）の単体テスト ──
//! NodeMetrics の serde 往復と、append→read のラウンドトリップ（一時 HARNESS_HOME）。

use thin_workflow_harness_core::metrics::{append_metrics, read_metrics, NodeMetrics};

#[test]
fn scripted_metrics_has_no_cost_or_tokens() {
    let m = NodeMetrics::scripted("node1", 4, 0.123);
    assert_eq!(m.node, "node1");
    assert_eq!(m.tool_calls, 4);
    assert!((m.wall_seconds - 0.123).abs() < 1e-9);
    assert!(m.cost.is_none());
    assert!(m.tokens.is_none());
    let json = serde_json::to_string(&m).unwrap();
    // cost / tokens は skip_serializing_if = None なので出力に含まれない。
    assert!(!json.contains("cost"), "json: {json}");
    assert!(!json.contains("tokens"), "json: {json}");
    let back: NodeMetrics = serde_json::from_str(&json).unwrap();
    assert_eq!(back, m);
}

#[test]
fn metrics_with_cost_and_tokens_roundtrips() {
    let m = NodeMetrics {
        node: "impl".into(),
        tool_calls: 12,
        wall_seconds: 3.5,
        cost: Some(0.0421),
        tokens: Some(8123),
        tokens_breakdown: None,
        ts: "2026-05-13T00:00:00Z".into(),
    };
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("cost"));
    assert!(json.contains("tokens"));
    let back: NodeMetrics = serde_json::from_str(&json).unwrap();
    assert_eq!(back, m);
}

#[test]
fn append_then_read_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    // append_metrics / read_metrics は HARNESS_HOME 配下の state/ を使う。
    // env を一時的に立てる ── このテストは serial 前提（他に env を触るテストは無い）。
    std::env::set_var("HARNESS_HOME", dir.path());
    let run = "20260513_000000";
    assert!(read_metrics(run).unwrap().is_none(), "no sidecar yet → None");
    append_metrics(run, &NodeMetrics::scripted("node1", 3, 0.01)).unwrap();
    append_metrics(run, &NodeMetrics::scripted("node2", 5, 0.02)).unwrap();
    let rows = read_metrics(run).unwrap().expect("sidecar exists");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].node, "node1");
    assert_eq!(rows[0].tool_calls, 3);
    assert_eq!(rows[1].node, "node2");
    assert_eq!(rows[1].tool_calls, 5);
    std::env::remove_var("HARNESS_HOME");
}
