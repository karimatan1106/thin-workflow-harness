//! workspace/symbol テスト用の最小 crate。
//! `User` 構造体と `create_user` 関数だけ ── どちらも `user` で hit する。

#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub name: String,
}

impl User {
    pub fn new(id: u64, name: String) -> Self {
        Self { id, name }
    }
}

pub fn create_user(id: u64, name: &str) -> User {
    User::new(id, name.to_string())
}

pub const MAX_USERS: u64 = 1000;
