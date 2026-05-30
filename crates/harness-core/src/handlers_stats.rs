//! `harness stats <run-id>` ── metrics サイドカーを読んでノードごと ＋ 合計 ＋
//! モデル別の集計を表示する（`DESIGN.md` §16.3）。
//!
//! ノード別行には model・tokens 内訳（input/output/cache_create/cache_read）・
//! cache hit 率を出す。さらにモデル別の tokens / cost / ノード数の集計を出す。
//! サイドカーは runtime ループ（`harness run --script`）が書く。debug CLI の
//! `advance` は書かない ── サイドカーが無ければ「メトリクス無し」と表示する。

use std::collections::BTreeMap;

use crate::metrics::{read_metrics, NodeMetrics};

pub fn cmd_stats(run_id: &str) -> Result<(), String> {
    let Some(rows) = read_metrics(run_id)? else {
        println!("run {run_id}: メトリクス無し（runtime ループ未実行 ── `harness run --script ...` で記録される）");
        return Ok(());
    };
    if rows.is_empty() {
        println!("run {run_id}: メトリクス行が空");
        return Ok(());
    }
    print_node_section(run_id, &rows);
    println!();
    print_model_section(&rows);
    Ok(())
}

/// cache hit 率 ── cache_read / (cache_read + input)。0 除算ガード。
/// cache_create は「新規書き込み」で hit ではないため分母に含めない。
pub fn cache_hit_rate(input: u64, cache_read: u64) -> f64 {
    let denom = input + cache_read;
    if denom == 0 {
        0.0
    } else {
        cache_read as f64 / denom as f64
    }
}

/// モデル別の集計エントリ。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelAgg {
    pub nodes: u64,
    pub tokens: u64,
    pub cost: f64,
}

/// モデルごとに tokens / cost / ノード数を集計する純関数。
/// model が None のノードは "(unknown)" にまとめる。BTreeMap でモデル名昇順。
pub fn aggregate_by_model(rows: &[NodeMetrics]) -> BTreeMap<String, ModelAgg> {
    let mut map: BTreeMap<String, ModelAgg> = BTreeMap::new();
    for r in rows {
        let key = r.model.clone().unwrap_or_else(|| "(unknown)".to_string());
        let e = map.entry(key).or_default();
        e.nodes += 1;
        e.tokens += r.tokens.unwrap_or(0);
        e.cost += r.cost.unwrap_or(0.0);
    }
    map
}

/// ノードごと ＋ 合計を表示する（cache 内訳・hit 率込み）。
fn print_node_section(run_id: &str, rows: &[NodeMetrics]) {
    println!("run {run_id} メトリクス（ノードごと）:");
    let (mut calls, mut wall, mut cost, mut tokens) = (0u64, 0.0f64, 0.0f64, 0u64);
    let (mut input, mut cread) = (0u64, 0u64);
    for m in rows {
        let bd = m.tokens_breakdown.clone().unwrap_or_default();
        let mut extra = String::new();
        if let Some(c) = m.cost {
            cost += c;
            extra.push_str(&format!("  cost={c:.4}"));
        }
        if let Some(t) = m.tokens {
            tokens += t;
            extra.push_str(&format!("  tokens={t}"));
        }
        if m.tokens_breakdown.is_some() {
            let hit = cache_hit_rate(bd.input, bd.cache_read) * 100.0;
            extra.push_str(&format!(
                "  [in={} out={} c_create={} c_read={} hit={hit:.1}%]",
                bd.input, bd.output, bd.cache_create, bd.cache_read
            ));
        }
        let model = m.model.as_deref().unwrap_or("-");
        println!(
            "  {:<16} model={:<22} tool_calls={:<4} wall={:.3}s{}  ({})",
            m.node, model, m.tool_calls, m.wall_seconds, extra, m.ts
        );
        calls += m.tool_calls;
        wall += m.wall_seconds;
        input += bd.input;
        cread += bd.cache_read;
    }
    let hit = cache_hit_rate(input, cread) * 100.0;
    println!(
        "  合計  tool_calls={calls}  wall={wall:.3}s  cost={cost:.4}  tokens={tokens}  cache_hit={hit:.1}%"
    );
}

/// モデル別の集計を表示する。
fn print_model_section(rows: &[NodeMetrics]) {
    println!("モデル別集計:");
    println!("  {:<24} {:>6} {:>12} {:>12}", "model", "nodes", "tokens", "cost");
    let agg = aggregate_by_model(rows);
    let (mut tn, mut tt, mut tc) = (0u64, 0u64, 0.0f64);
    for (model, e) in &agg {
        println!(
            "  {:<24} {:>6} {:>12} {:>12.4}",
            model, e.nodes, e.tokens, e.cost
        );
        tn += e.nodes;
        tt += e.tokens;
        tc += e.cost;
    }
    println!("  {:<24} {:>6} {:>12} {:>12.4}", "合計", tn, tt, tc);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::TokenBreakdown;

    fn mk(node: &str, model: Option<&str>, tokens: u64, cost: f64) -> NodeMetrics {
        let b = TokenBreakdown { input: tokens, output: 0, cache_create: 0, cache_read: 0 };
        NodeMetrics::api(node, 1, 1.0, b, Some(cost), model.map(|s| s.to_string()))
    }

    #[test]
    fn hit_rate_zero_guard() {
        assert_eq!(cache_hit_rate(0, 0), 0.0);
    }

    #[test]
    fn hit_rate_basic() {
        // cache_read=75, input=25 → 75/100 = 0.75
        assert!((cache_hit_rate(25, 75) - 0.75).abs() < 1e-9);
    }

    #[test]
    fn aggregate_groups_by_model_and_unknown() {
        let rows = vec![
            mk("a", Some("opus"), 100, 0.10),
            mk("b", Some("opus"), 50, 0.05),
            mk("c", Some("haiku"), 10, 0.01),
            mk("d", None, 7, 0.00),
        ];
        let agg = aggregate_by_model(&rows);
        assert_eq!(agg.len(), 3);
        let opus = &agg["opus"];
        assert_eq!(opus.nodes, 2);
        assert_eq!(opus.tokens, 150);
        assert!((opus.cost - 0.15).abs() < 1e-9);
        assert_eq!(agg["haiku"].nodes, 1);
        assert_eq!(agg["(unknown)"].nodes, 1);
        assert_eq!(agg["(unknown)"].tokens, 7);
    }
}
