//! Python の test 関数判定（heuristic）。
//!
//! pytest 慣習に従う最小実装。本格的な tree-sitter 解析 / pytest fixture は
//! 次バッチで強化予定（refs/callers/closure/test 判定強化と同タイミング）。
//!
//! 判定基準:
//! - file path: `test_<name>.py` / `<name>_test.py` / `tests/` 配下
//! - 関数 / メソッド名: `test_*` / `*_test` / `test`

/// path + name の OR で test ノードかを判定。
pub fn is_test_node_py(name: &str, file: &str) -> bool {
    is_test_file_py(file) || is_test_name_py(name)
}

/// pytest discovery のファイル名規約に従う path 判定。
pub fn is_test_file_py(file: &str) -> bool {
    let norm = file.replace('\\', "/");
    let leaf = norm.rsplit('/').next().unwrap_or(&norm);
    if leaf.starts_with("test_") && leaf.ends_with(".py") {
        return true;
    }
    if leaf.ends_with("_test.py") {
        return true;
    }
    norm.starts_with("tests/") || norm.contains("/tests/")
}

/// 関数 / メソッド名から test 関数かどうかを判定。
pub fn is_test_name_py(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let leaf = match name.rsplit_once('.') {
        Some((_, leaf)) => leaf,
        None => name,
    };
    leaf.starts_with("test_") || leaf.ends_with("_test") || leaf == "test"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn py_test_file_path_heuristic() {
        assert!(is_test_file_py("tests/test_user.py"));
        assert!(is_test_file_py("src/test_user.py"));
        assert!(is_test_file_py("src/user_test.py"));
        assert!(is_test_file_py("tests/sub/foo.py"));
        assert!(is_test_file_py("pkg/tests/foo.py"));
        assert!(!is_test_file_py("src/user.py"));
        assert!(!is_test_file_py("src/testing.py"));
    }

    #[test]
    fn py_test_name_heuristic() {
        assert!(is_test_name_py("test_create_user"));
        assert!(is_test_name_py("create_user_test"));
        assert!(is_test_name_py("test"));
        assert!(is_test_name_py("TestUser.test_create"));
        assert!(!is_test_name_py("create_user"));
        assert!(!is_test_name_py("testing"));
        assert!(!is_test_name_py(""));
    }

    #[test]
    fn py_combined_node() {
        // file が test、name が非 test でも hit
        assert!(is_test_node_py("create_user", "tests/test_foo.py"));
        // name が test、file が非 test でも hit
        assert!(is_test_node_py("test_create_user", "src/user.py"));
        // どちらでもないと miss
        assert!(!is_test_node_py("create_user", "src/user.py"));
    }
}
