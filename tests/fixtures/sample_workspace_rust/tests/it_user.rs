//! integration test ── `create_user` を呼ぶ test 関数を 1 つだけ用意。
//! `tested_by_test_filter` の test ノード fixture。

use sample_workspace_rust::create_user;

#[test]
fn test_create_user() {
    let u = create_user(42, "test");
    assert_eq!(u.id, 42);
    assert_eq!(u.name, "test");
}
