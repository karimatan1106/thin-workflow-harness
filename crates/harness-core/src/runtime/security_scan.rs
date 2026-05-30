//! コスト0 静的セキュリティパターン検知（security-guidance プラグイン 層1 の移植）。
//!
//! edit_file / create_file の content を**ローカル substring 照合**でスキャンし、
//! 既知の危険パターンに warning を返す。LLM は呼ばない（依存ゼロ・即時）ので
//! 「コスト0」。あくまで非ブロッキングの注意喚起 ── 書き込み自体は止めない
//! （止めるのは blast_radius / path traversal / security node の exit_gate の役割）。
//!
//! 本家 `patterns.py`(25 ルール, regex 主体) から、誤検出を抑えつつ高価値なものを
//! substring で移植した curated subset。クラス: injection / XSS / 危険デシリアライズ /
//! 弱い暗号 / TLS 無効化 / hardcoded secret。

/// 1 件の検出結果。
pub struct Finding {
    pub rule: &'static str,
    pub message: &'static str,
}

const JS_EXTS: &[&str] = &[".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs", ".mts", ".cts", ".vue", ".svelte"];
const PY_EXTS: &[&str] = &[".py", ".pyi"];

fn has_ext(path: &str, exts: &[&str]) -> bool {
    let p = path.to_ascii_lowercase();
    exts.iter().any(|e| p.ends_with(e))
}

/// 1 ルール定義 ── 対象拡張子（None=全言語）と「マッチに必要な substring 群（OR）」。
struct Rule {
    name: &'static str,
    exts: Option<&'static [&'static str]>,
    needles: &'static [&'static str],
    message: &'static str,
}

const RULES: &[Rule] = &[
    // ── injection ──
    Rule { name: "eval_injection", exts: None, needles: &["eval("],
        message: "eval() は任意コード実行。JSON.parse / ast.literal_eval / 安全な式パーサに置換せよ。安全と判断するならコメントで根拠を残せ。" },
    Rule { name: "js_new_function", exts: Some(JS_EXTS), needles: &["new Function("],
        message: "new Function() への文字列補間はコード injection。obj[key] や式パーサで代替せよ。" },
    Rule { name: "js_child_process_exec", exts: Some(JS_EXTS), needles: &["child_process.exec", "execSync("],
        message: "child_process.exec は shell 経由で command injection。execFile/spawn を引数配列で使え。" },
    Rule { name: "py_os_system", exts: Some(PY_EXTS), needles: &["os.system("],
        message: "os.system は shell 起動の command injection sink。subprocess.run([...]) を使え。" },
    Rule { name: "py_subprocess_shell", exts: Some(PY_EXTS), needles: &["shell=True"],
        message: "subprocess(shell=True) は command injection。引数を list で渡し shell を介すな。" },
    // ── XSS ──
    Rule { name: "react_dangerous_html", exts: Some(JS_EXTS), needles: &["dangerouslySetInnerHTML"],
        message: "dangerouslySetInnerHTML は XSS。DOMPurify でサニタイズするか安全な代替を使え。" },
    Rule { name: "dom_innerhtml", exts: Some(JS_EXTS), needles: &[".innerHTML =", ".innerHTML=", ".outerHTML =", ".outerHTML="],
        message: "innerHTML/outerHTML 代入は XSS sink。textContent か DOMPurify を使え。" },
    Rule { name: "dom_document_write", exts: Some(JS_EXTS), needles: &["document.write"],
        message: "document.write は XSS + 性能問題。createElement/appendChild を使え。" },
    // ── 危険デシリアライズ ──
    Rule { name: "py_pickle_load", exts: Some(PY_EXTS), needles: &["pickle.load", "cPickle.load", "cloudpickle.load", "dill.load", "marshal.load"],
        message: "untrusted な pickle/marshal 等の load は任意コード実行。JSON か schema 検証デシリアライザを使え。" },
    Rule { name: "py_yaml_load", exts: Some(PY_EXTS), needles: &["yaml.load(", "yaml.unsafe_load"],
        message: "yaml.load/unsafe_load は !!python/object で任意コード実行。yaml.safe_load を使え。" },
    Rule { name: "py_torch_load", exts: Some(PY_EXTS), needles: &["torch.load("],
        message: "torch.load は既定 weights_only=False で unpickle。weights_only=True を渡せ。" },
    // ── 弱い暗号 / TLS ──
    Rule { name: "weak_hash_md5", exts: None, needles: &["md5(", "MD5(", "hashlib.md5", "createHash('md5'", "createHash(\"md5\""],
        message: "MD5/SHA1 は衝突可能で署名・パスワード用途に不適。SHA-256 以上か bcrypt/argon2 を使え。" },
    Rule { name: "aes_ecb", exts: None, needles: &["MODE_ECB", "aes-128-ecb", "aes-256-ecb"],
        message: "AES-ECB は平文構造が漏れる。AES-GCM か AES-CBC+HMAC を使え。" },
    Rule { name: "node_createcipher", exts: Some(JS_EXTS), needles: &["createCipher(", "createDecipher("],
        message: "crypto.createCipher は IV なし・MD5 KDF。createCipheriv/createDecipheriv を使え。" },
    Rule { name: "tls_verify_disabled", exts: None, needles: &["verify=False", "rejectUnauthorized: false", "rejectUnauthorized:false", "InsecureSkipVerify: true", "NODE_TLS_REJECT_UNAUTHORIZED", "_create_unverified_context"],
        message: "TLS 検証無効化は MITM を許す。CA を信頼ストアに入れるか正規証明書を使え。" },
    // ── hardcoded secret ──
    Rule { name: "hardcoded_aws_key", exts: None, needles: &["AKIA"],
        message: "AWS アクセスキー(AKIA…)がハードコードされている疑い。env / secret manager に移せ。" },
    Rule { name: "private_key_block", exts: None, needles: &["-----BEGIN PRIVATE KEY-----", "-----BEGIN RSA PRIVATE KEY-----", "-----BEGIN OPENSSH PRIVATE KEY-----"],
        message: "秘密鍵がファイルに埋め込まれている。リポジトリから除去し secret manager で管理せよ。" },
];

/// content を全ルールでスキャンし、マッチした findings を返す（コスト0・非ブロッキング）。
pub fn scan(path: &str, content: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    for rule in RULES {
        if let Some(exts) = rule.exts {
            if !has_ext(path, exts) {
                continue;
            }
        }
        if rule.needles.iter().any(|n| content.contains(n)) {
            out.push(Finding { rule: rule.name, message: rule.message });
        }
    }
    out
}

/// findings を worker 向けの 1 つの warning 文字列に整形する。空なら None。
pub fn format_warning(path: &str, findings: &[Finding]) -> Option<String> {
    if findings.is_empty() {
        return None;
    }
    let mut s = format!("⚠️ security 静的検知（{}）── 書込は許可したが要確認:", path);
    for f in findings {
        s.push_str(&format!("\n  - [{}] {}", f.rule, f.message));
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_python_eval_and_pickle() {
        let f = scan("a.py", "x = eval(user_in)\nimport pickle\npickle.load(fp)");
        let names: Vec<_> = f.iter().map(|x| x.rule).collect();
        assert!(names.contains(&"eval_injection"));
        assert!(names.contains(&"py_pickle_load"));
    }

    #[test]
    fn js_rules_gated_by_extension() {
        // .py には JS ルール(innerHTML)は出ない。
        let f = scan("a.py", "el.innerHTML = x");
        assert!(!f.iter().any(|x| x.rule == "dom_innerhtml"));
        // .ts には出る。
        let f2 = scan("a.ts", "el.innerHTML = x");
        assert!(f2.iter().any(|x| x.rule == "dom_innerhtml"));
    }

    #[test]
    fn detects_secrets_any_language() {
        let f = scan("config.rs", "const K = \"AKIAIOSFODNN7EXAMPLE\";");
        assert!(f.iter().any(|x| x.rule == "hardcoded_aws_key"));
    }

    #[test]
    fn clean_content_yields_nothing() {
        let f = scan("a.py", "def add(a, b):\n    return a + b\n");
        assert!(f.is_empty());
        assert!(format_warning("a.py", &f).is_none());
    }

    #[test]
    fn warning_lists_each_finding() {
        let f = scan("a.py", "yaml.load(s)\nos.system(c)");
        let w = format_warning("a.py", &f).expect("findings あり");
        assert!(w.contains("py_yaml_load"));
        assert!(w.contains("py_os_system"));
    }
}
