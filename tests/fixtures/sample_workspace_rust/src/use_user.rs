//! `create_user` / `User::new` への参照と呼び出しを作る ── refs / callers テスト用。

use crate::{create_user, User};

pub fn make_alice() -> User {
    create_user(1, "alice")
}

pub fn make_bob() -> User {
    create_user(2, "bob")
}

pub fn make_direct() -> User {
    User::new(3, "carol".to_string())
}
