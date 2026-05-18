//! 暴走防止ガード群 ── `run_loop` から切り出した責務分離モジュール。
//!
//! - `BudgetStreak` ── 同ノードでの連続 `BudgetExceeded` 検出（2 連続で early-fail）。
//! - `recommend_max_tokens` ── 早期 fail メッセージに「現在値→推奨値」を埋める helper。
//! - `should_emit_cache_warning` ── `cache_create=0` 警告を 1 run につき 1 度だけ出す判定。

use crate::runtime::api_worker::{ApiWorkerMetrics, Outcome};
use crate::workflow::Budget;

/// 同ノードで連続して `BudgetExceeded` が起きた回数を追跡する。
/// 別ノードに進むか non-budget の outcome が出たら count をリセット。
pub(super) struct BudgetStreak {
    node_id: Option<String>,
    count: u32,
}

impl BudgetStreak {
    /// 連続到達数の閾値 ── これを超えたら early-fail。
    pub(super) const MAX_CONSECUTIVE: u32 = 2;

    pub(super) fn new() -> Self { Self { node_id: None, count: 0 } }

    /// ノード進入時に呼ぶ ── 別ノードに切り替わったら streak をリセット。
    pub(super) fn enter(&mut self, node_id: &str) {
        if self.node_id.as_deref() != Some(node_id) {
            self.node_id = Some(node_id.to_string());
            self.count = 0;
        }
    }

    /// `BudgetExceeded` なら +1、他は 0 にリセット。早期 fail すべきなら `true`。
    pub(super) fn observe_outcome(&mut self, outcome: &Outcome) -> bool {
        if matches!(outcome, Outcome::BudgetExceeded(_)) {
            self.count += 1;
            self.count >= Self::MAX_CONSECUTIVE
        } else {
            self.count = 0;
            false
        }
    }

    pub(super) fn count(&self) -> u32 { self.count }
}

/// `max_tokens` 推奨値（現在値の倍）── 早期 fail メッセージで人間に具体値を提示する。
pub(super) fn recommend_max_tokens(budget: &Budget) -> String {
    match budget.max_tokens {
        Some(cur) => format!(
            "現在 max_tokens={cur} ── 推奨: 倍の {} に引き上げ",
            cur.saturating_mul(2)
        ),
        None => "現在 max_tokens 未設定 ── workflow.toml の budget に max_tokens=16000 程度を明示せよ"
            .to_string(),
    }
}

/// cache 未作成の警告を出すべきか（API を 1 回でも呼んでいて、cache_create も cache_read も 0）。
pub(super) fn should_emit_cache_warning(metrics: &ApiWorkerMetrics) -> bool {
    metrics.api_calls > 0
        && metrics.usage.cache_creation_input_tokens == 0
        && metrics.usage.cache_read_input_tokens == 0
}

/// 警告本文（呼び出し側で 1 run につき 1 度だけ表示する）。
pub(super) fn cache_warning_message() -> &'static str {
    "[warning] cache_create=0 / cache_read=0 ── prompt cache が作成されていない。\n  真因候補: \
     (a) system+tools の合計 input が 1024 token 閾値未達、\
     (b) anthropic-beta に prompt-caching-2024-07-31 欠落、\
     (c) system block の cache_control マーカー不在。\n  \
     修正前にこの警告に必ず対処すること（以降は警告を抑制）。"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::anthropic::Usage;

    fn budget_exceeded() -> Outcome {
        Outcome::BudgetExceeded("max_tokens=4000 に到達".into())
    }

    #[test]
    fn streak_increments_only_on_budget_exceeded() {
        let mut s = BudgetStreak::new();
        s.enter("n1");
        // 1 回目 → まだ閾値未達。
        assert!(!s.observe_outcome(&budget_exceeded()));
        assert_eq!(s.count(), 1);
        // 2 回目 → 早期 fail シグナル。
        assert!(s.observe_outcome(&budget_exceeded()));
        assert_eq!(s.count(), 2);
    }

    #[test]
    fn streak_resets_on_node_change() {
        let mut s = BudgetStreak::new();
        s.enter("n1");
        s.observe_outcome(&budget_exceeded());
        assert_eq!(s.count(), 1);
        s.enter("n2");
        assert_eq!(s.count(), 0);
    }

    #[test]
    fn streak_resets_on_non_budget_outcome() {
        let mut s = BudgetStreak::new();
        s.enter("n1");
        s.observe_outcome(&budget_exceeded());
        // transitioned 等で streak が切れる。
        assert!(!s.observe_outcome(&Outcome::Transitioned));
        assert_eq!(s.count(), 0);
    }

    #[test]
    fn recommend_max_tokens_doubles_current() {
        let b = Budget { max_tokens: Some(4000), ..Default::default() };
        let r = recommend_max_tokens(&b);
        assert!(r.contains("4000"), "{r}");
        assert!(r.contains("8000"), "倍値 8000 が含まれない: {r}");
    }

    #[test]
    fn recommend_max_tokens_handles_unset() {
        let b = Budget { max_tokens: None, ..Default::default() };
        let r = recommend_max_tokens(&b);
        assert!(r.contains("未設定"), "{r}");
    }

    #[test]
    fn cache_warning_requires_api_call_and_zero_cache() {
        let mut m = ApiWorkerMetrics::default();
        // API 呼び出し前 → 警告不要。
        assert!(!should_emit_cache_warning(&m));
        m.api_calls = 1;
        // cache_create=0 / cache_read=0 → 警告。
        assert!(should_emit_cache_warning(&m));
        // cache_read > 0 → 警告不要（cache hit している）。
        m.usage = Usage { cache_read_input_tokens: 100, ..Default::default() };
        assert!(!should_emit_cache_warning(&m));
        // cache_create > 0 でも警告不要。
        m.usage = Usage { cache_creation_input_tokens: 100, ..Default::default() };
        assert!(!should_emit_cache_warning(&m));
    }
}
