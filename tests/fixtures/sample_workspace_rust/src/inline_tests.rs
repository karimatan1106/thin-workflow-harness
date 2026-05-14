//! `#[cfg(test)] mod` 内側の `#[test]` 関数 fixture（attr ベース検出用）。
//!
//! 既存 fixture は tests/ 配下に置いた integration test のみだったので、
//! file path heuristic が無くても attr だけで test 判定できることを示すため
//! src/ 配下に inline test を 1 つ追加する。call hierarchy が成立するよう
//! create_user / User::new を呼ぶ。

use crate::{create_user, User};

/// テスト用の薄いラッパ（attr 無し helper）。`test_inline` から呼ばれる。
pub fn build_inline_user() -> User {
    create_user(99, "inline")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline() {
        let u = build_inline_user();
        assert_eq!(u.id, 99);
        assert_eq!(u.name, "inline");
    }
}
