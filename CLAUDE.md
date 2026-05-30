# thin-workflow-harness

Thin workflow runner for LLM agent loops (Rust). `harness` CLI binary が `workflow.toml` + `.harness/skills/*.md` を駆動する。

## Crate names (cargo -p)

workspace の crate 名は binary/lib で接頭辞が付く:

- `thin-workflow-harness` — binary (`harness.exe`)
- `thin-workflow-harness-core` — lib (runtime / gate / scaffold)

`cargo build -p harness-core` は `did not match any packages` で失敗する。正しくは:

```
cargo build -p thin-workflow-harness-core
cargo test  -p thin-workflow-harness --test <name>   # 統合テストは binary crate 配下
```
