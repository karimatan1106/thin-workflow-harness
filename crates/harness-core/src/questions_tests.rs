//! `validate_answer` の単体テスト（`questions.rs` から `#[path]` で取り込む）。

use super::*;
use crate::gate::Question;

/// テスト用の Question を組み立てるヘルパ。
fn q(options: &[&str]) -> Question {
    Question {
        id: "q1".into(),
        kind: "clarify".into(),
        question: "Which?".into(),
        header: "".into(),
        options: options.iter().map(|s| s.to_string()).collect(),
        required: true,
        context_ref: None,
        answered: false,
        answer: None,
    }
}

#[test]
fn free_form_always_ok() {
    // options 空なら任意の回答を許す（後方互換）
    assert!(validate_answer(&q(&[]), "anything").is_ok());
}

#[test]
fn accepts_valid_option() {
    assert!(validate_answer(&q(&["A", "B"]), "A").is_ok());
    // 前後空白は trim して比較
    assert!(validate_answer(&q(&["A", "B"]), " B ").is_ok());
}

#[test]
fn rejects_invalid_option() {
    let e = validate_answer(&q(&["A", "B"]), "C").unwrap_err();
    assert!(e.contains("無効"));
    // エラーに有効な選択肢を提示する
    assert!(e.contains("A, B"));
}
