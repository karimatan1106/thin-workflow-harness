//! Outline test fixture ── 各 SymbolKind を含む小さい Rust ファイル。

const MAX_USERS: usize = 100;

static GREETING: &str = "hi";

fn main() {
    println!("hello");
}

pub struct User {
    pub name: String,
    pub age: u32,
}

impl User {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), age: 0 }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

pub enum Status {
    Active,
    Inactive,
}

pub trait Greet {
    fn greet(&self) -> String;
}

pub mod inner {
    pub fn helper() -> u32 {
        42
    }
}
