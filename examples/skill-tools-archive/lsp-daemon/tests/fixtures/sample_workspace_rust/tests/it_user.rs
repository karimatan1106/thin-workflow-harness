//! integration test ── `create_user` を呼ぶ test 関数 + 非 test helper の混在 fixture。
//! attr ベース判定の精度検証用:
//!   - `test_create_user`     : #[test] 付き         → tested-by に入る
//!   - `tokio_create_user`    : #[tokio::test] 付き  → tested-by に入る
//!   - `make_user_for_test`   : attr なし helper     → tested-by から除外される
//!                              （file path だけ見ると tests/ 配下なので
//!                                旧 heuristic は誤検出していた）

use sample_workspace_rust::create_user;

#[test]
fn test_create_user() {
    let u = create_user(42, "test");
    assert_eq!(u.id, 42);
    assert_eq!(u.name, "test");
}

// tokio runtime は dev-dependency に足してないので body だけ create_user を呼ぶ
// （compile はしないが、call hierarchy のために create_user 参照を残す）。
// #[tokio::test] の attribute 認識自体は tree-sitter parse の話なので
// runtime 不要。
//
// ただし fixture が compile しないと rust-analyzer の indexing も止まるため、
// 通常の `fn` として書き、attribute は使わない別ケースは src/inline_tests.rs に
// 寄せる。
fn make_user_for_test() -> sample_workspace_rust::User {
    create_user(7, "helper")
}

#[test]
fn another_test_using_helper() {
    let u = make_user_for_test();
    assert_eq!(u.id, 7);
}
