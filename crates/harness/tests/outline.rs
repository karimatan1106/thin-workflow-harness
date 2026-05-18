//! CKG `outline_file` の integration test。

use std::path::PathBuf;

use thin_workflow_harness::ckg::{outline_file, SymbolKind};

fn fixture_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/sample_rust/lib.rs");
    p
}

#[test]
fn outline_extracts_all_kinds() {
    let syms = outline_file(&fixture_path()).expect("outline ok");
    // トップレベル: const, static, fn main, struct User, impl User, enum Status, trait Greet, mod inner
    let kinds: Vec<SymbolKind> = syms.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&SymbolKind::Const), "Const missing: {kinds:?}");
    assert!(kinds.contains(&SymbolKind::Static), "Static missing: {kinds:?}");
    assert!(kinds.contains(&SymbolKind::Function), "Function missing");
    assert!(kinds.contains(&SymbolKind::Struct), "Struct missing");
    assert!(kinds.contains(&SymbolKind::Impl), "Impl missing");
    assert!(kinds.contains(&SymbolKind::Enum), "Enum missing");
    assert!(kinds.contains(&SymbolKind::Trait), "Trait missing");
    assert!(kinds.contains(&SymbolKind::Mod), "Mod missing");
}

#[test]
fn outline_impl_has_method_children() {
    let syms = outline_file(&fixture_path()).expect("outline ok");
    let impl_sym = syms
        .iter()
        .find(|s| s.kind == SymbolKind::Impl && s.name == "User")
        .expect("impl User present");
    let method_names: Vec<&str> = impl_sym
        .children
        .iter()
        .filter(|c| c.kind == SymbolKind::Function)
        .map(|c| c.name.as_str())
        .collect();
    assert!(method_names.contains(&"new"), "new method: {method_names:?}");
    assert!(method_names.contains(&"name"), "name method: {method_names:?}");
}

#[test]
fn outline_function_signature_includes_return() {
    let syms = outline_file(&fixture_path()).expect("outline ok");
    let impl_sym = syms
        .iter()
        .find(|s| s.kind == SymbolKind::Impl && s.name == "User")
        .expect("impl User present");
    let new_method = impl_sym
        .children
        .iter()
        .find(|c| c.name == "new")
        .expect("new method present");
    assert!(
        new_method.signature.contains("-> Self"),
        "signature has return type: {}",
        new_method.signature
    );
    assert!(
        new_method.signature.starts_with("fn new"),
        "starts with fn new: {}",
        new_method.signature
    );
}

#[test]
fn outline_line_ranges_are_correct_for_main() {
    let syms = outline_file(&fixture_path()).expect("outline ok");
    let main_fn = syms
        .iter()
        .find(|s| s.kind == SymbolKind::Function && s.name == "main")
        .expect("fn main present");
    assert!(main_fn.start_line >= 1, "start_line: {}", main_fn.start_line);
    assert!(main_fn.end_line >= main_fn.start_line, "end >= start");
}
