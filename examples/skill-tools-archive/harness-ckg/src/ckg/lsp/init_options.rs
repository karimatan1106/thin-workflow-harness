//! LSP initialize 時の initializationOptions ── Lang ごとに最適化された設定。
//! TS は tsserver preferences で cold init 短縮を狙う。
//!
//! 各 preference の意図:
//! - disableSourceOfProjectReferenceRedirect: monorepo project reference 経由参照を抑制し indexing 軽量化
//! - includePackageJsonAutoImports="off": auto-import 用 node_modules 全 scan を停止 (cold init 主因)
//! - includeCompletionsForModuleExports=false: 補完用の export スキャンを停止
//! - providePrefixAndSuffixTextForRename=false: rename 補助情報を省略
//! - allowIncompleteCompletions=true: indexing 進行中でも応答する
//! - useSyntaxServer="auto": syntax-only server を併用、semantic 操作は on-demand
//! - maxTsServerMemory=4096: default 3072MB → 4096MB で swap 回避
//! - logVerbosity="off": tsserver ログ抑制
//!
//! 効果計測は別バッチ (15min+ 要する) で実施。本ファイルでは default 固定値のみ提供する。

use serde_json::{json, Value};

use super::lang::Lang;

/// Lang 別の initializationOptions を返す。
/// 該当なし or 空でよい言語は `Value::Null` を返す。
pub fn init_options(lang: Lang) -> Value {
    match lang {
        // rust-analyzer は default で十分。capabilities 経由で別途設定する。
        Lang::Rust => Value::Null,
        Lang::Ts => ts_init_options(),
        // pyright は default で十分。
        Lang::Py => Value::Null,
        // gopls は default で十分。
        Lang::Go => Value::Null,
    }
}

fn ts_init_options() -> Value {
    json!({
        "hostInfo": "harness",
        "preferences": {
            "disableSourceOfProjectReferenceRedirect": true,
            "includePackageJsonAutoImports": "off",
            "includeCompletionsForModuleExports": false,
            "providePrefixAndSuffixTextForRename": false,
            "allowIncompleteCompletions": true
        },
        "tsserver": {
            "useSyntaxServer": "auto",
            "maxTsServerMemory": 4096,
            "logVerbosity": "off"
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ts_init_options_has_required_fields() {
        let v = init_options(Lang::Ts);
        assert_eq!(v["hostInfo"], "harness");
        // preferences
        let prefs = &v["preferences"];
        assert_eq!(prefs["disableSourceOfProjectReferenceRedirect"], true);
        assert_eq!(prefs["includePackageJsonAutoImports"], "off");
        assert_eq!(prefs["includeCompletionsForModuleExports"], false);
        assert_eq!(prefs["providePrefixAndSuffixTextForRename"], false);
        assert_eq!(prefs["allowIncompleteCompletions"], true);
        // tsserver
        let ts = &v["tsserver"];
        assert_eq!(ts["useSyntaxServer"], "auto");
        assert_eq!(ts["maxTsServerMemory"], 4096);
        assert_eq!(ts["logVerbosity"], "off");
    }

    #[test]
    fn rust_init_options_is_null() {
        assert_eq!(init_options(Lang::Rust), Value::Null);
    }

    #[test]
    fn py_init_options_is_null() {
        assert_eq!(init_options(Lang::Py), Value::Null);
    }

    #[test]
    fn go_init_options_is_null() {
        assert_eq!(init_options(Lang::Go), Value::Null);
    }
}
