# skill-tools

skill repo template として配置される CKG tool 群。harness 本体 (workflow runner) からは独立。

## 含まれるもの

- `harness-ckg/` ── CKG library (tree-sitter outline + LSP wrap + daemon protocol)
- `lsp-daemon/` ── LSP daemon + CKG primitive binary (`harness-lspd.exe` 生成)

## 使い方

skill repo の中で「rust なら rust-analyzer 経由で symbol を引け」のような instruction を書くとき、これらの tool を skill repo に同梱 or user が cargo install で別途 build する。

```bash
cargo install --path crates/skill-tools/lsp-daemon
```

これで `harness-lspd.exe` が PATH に追加される。skill 内で `run_command("harness-lspd find-symbol X --lang rust ...")` を呼べる。

## 思想

harness 本体 (workflow runner) は CKG tool の存在を知らない。これらは skill が抱える tool であり、user/skill 作者の責任で install / 維持する。

L3 (配布境界) として workspace から完全分離、`cargo build --workspace` では build されない。skill-tools の build は次のように行う:

```bash
cargo build --release --manifest-path crates/skill-tools/lsp-daemon/Cargo.toml
```
