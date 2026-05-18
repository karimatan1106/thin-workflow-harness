# investigate example

2 phase の CKG tool 使用例。 `harness-lspd` で symbol 引きを実行し、 結果を artifact 登録して終了する。

## 構造

```
investigate/
├── README.md
└── .harness/
    ├── workflow.toml             ← 2 node (explore → report)
    └── skills/
        ├── 01-explore.md         ← outline + find-symbol → record_artifact
        └── 02-report.md          ← read_file artifact → report_evidence
```

## 前提 install

CKG primitive CLI (`harness-lspd`) が必要:

```bash
cd /path/to/thin-workflow-harness
cargo install --path examples/skill-tools-archive/lsp-daemon
# → harness-lspd が ~/.cargo/bin/ に install される
```

確認:

```bash
harness-lspd --help
```

## 動かす

```bash
# 1. このディレクトリを cwd にする (Rust workspace 想定)
cd /path/to/some-rust-workspace

# 2. このリポの .harness/ を cp
cp -r /path/to/thin-workflow-harness/examples/skill-repos/investigate/.harness .

# 3. run 開始
harness start "investigate codebase"
harness validate

# 4. runtime ループ ── ANTHROPIC_API_KEY 必須
export ANTHROPIC_API_KEY=sk-ant-...
harness run --model claude-haiku-4-5
```

## 期待結果

- 2 ノード遷移 (`explore` → `report`)
- artifact `exploration_summary` が登録される
- evidence `completed` が登録される
- workflow 終了 (next=[])

## 設計意図

harness 本体 (workflow runner) は CKG tool の存在を知らない。 CKG は **skill が抱える tool** であり、 user/skill 作者が install / 維持する (examples/skill-tools-archive/README.md §「思想」)。
