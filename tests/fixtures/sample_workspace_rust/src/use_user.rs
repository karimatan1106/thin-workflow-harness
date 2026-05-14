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

pub fn make_pair() -> (User, User) {
    (make_alice(), make_bob())
}

pub fn make_party() -> Vec<User> {
    let (a, b) = make_pair();
    vec![a, b, make_direct()]
}
