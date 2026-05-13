//! `harness stats <run-id>` ── metrics サイドカーを読んでノードごと ＋ 合計の
//! tool_calls / wall_seconds（＋ あれば cost / tokens）を表示する（`DESIGN.md` §16.3）。
//!
//! サイドカーは runtime ループ（`harness run --script`）が書く。debug CLI の `advance` は書かない
//! ── サイドカーが無ければ「メトリクス無し（runtime ループ未実行）」と表示する。

use crate::metrics::read_metrics;

pub fn cmd_stats(run_id: &str) -> Result<(), String> {
    let Some(rows) = read_metrics(run_id)? else {
        println!("run {run_id}: メトリクス無し（runtime ループ未実行 ── `harness run --script ...` で記録される）");
        return Ok(());
    };
    if rows.is_empty() {
        println!("run {run_id}: メトリクス行が空");
        return Ok(());
    }
    println!("run {run_id} メトリクス（ノードごと）:");
    let mut total_calls: u64 = 0;
    let mut total_wall: f64 = 0.0;
    let mut total_cost: f64 = 0.0;
    let mut total_tokens: u64 = 0;
    let mut any_cost = false;
    let mut any_tokens = false;
    for m in &rows {
        total_calls += m.tool_calls;
        total_wall += m.wall_seconds;
        let mut extra = String::new();
        if let Some(c) = m.cost {
            total_cost += c;
            any_cost = true;
            extra.push_str(&format!("  cost={c:.4}"));
        }
        if let Some(t) = m.tokens {
            total_tokens += t;
            any_tokens = true;
            extra.push_str(&format!("  tokens={t}"));
        }
        println!(
            "  {:<16} tool_calls={:<4} wall={:.3}s{}  ({})",
            m.node, m.tool_calls, m.wall_seconds, extra, m.ts
        );
    }
    let mut tline = format!("  合計  tool_calls={total_calls}  wall={total_wall:.3}s");
    if any_cost {
        tline.push_str(&format!("  cost={total_cost:.4}"));
    }
    if any_tokens {
        tline.push_str(&format!("  tokens={total_tokens}"));
    }
    println!("{tline}");
    Ok(())
}
