//! `#[cfg(test)] mod` 内側の `#[test]` 関数 fixture（attr ベース検出用）。
//!
//! 既存 fixture は tests/ 配下に置いた integration test のみだったので、
//! file path heuristic が無くても attr だけで test 判定できることを示すため
//! src/ 配下に inline test を 1 つ追加する。call hierarchy が成立するよう
//! create_user / User::new を呼ぶ。
//!
//! 加えて、 attr 無し helper（`helper_no_attr`）も同じ `#[cfg(test)] mod`
//! 内側に置く。これは attr 直接検出では拾えないが、`is_inside_cfg_test_mod`
//! による親階層判定で test とみなされるべき関数 ── tested-by の 2 段目
//! 判定（cfg(test) mod 内側）の本番 fixture。

use crate::{create_user, User};

/// テスト用の薄いラッパ（attr 無し helper）。`test_inline` から呼ばれる。
pub fn build_inline_user() -> User {
    create_user(99, "inline")
}

#[cfg(test)]
mod tests {
    use super::*;

    // attr 付き ── 既存検出ロジック（test_attrs.rs）でも通る。
    #[test]
    fn test_inline() {
        let u = build_inline_user();
        assert_eq!(u.id, 99);
        assert_eq!(u.name, "inline");
    }

    // attr 無し helper ── `#[test]` が無いので attr 直接検出では拾えない。
    // ただし `#[cfg(test)] mod tests` の内側に居るので
    // `is_inside_cfg_test_mod` 経由で test とみなされるはず（新ロジックの本丸）。
    // call hierarchy が成立するよう User::new を呼ぶ。
    fn helper_no_attr() {
        // create_user 経由で User を作る ── tested-by の起点 qname `create_user`
        // から call hierarchy で辿れるようにする。
        let _u = create_user(7, "helper");
    }

    #[test]
    fn test_uses_helper_no_attr() {
        helper_no_attr();
    }
}
