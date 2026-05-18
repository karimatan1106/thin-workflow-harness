# minimal example

最小 example。 1 phase × `report_evidence` のみ。 CKG tool 使用なし。

## 構造

```
minimal/
├── README.md            ← このファイル
└── .harness/
    ├── workflow.toml    ← 1 node (`done`) のみ
    └── skills/
        └── 01-done.md   ← worker に渡される skill 文面
```

## 動かす

```bash
# 1. このディレクトリを cwd にする
cd examples/skill-repos/minimal

# 2. run 開始
harness start "minimal example"

# 3. workflow.toml の static 検証 (オプション)
harness validate

# 4. runtime ループ ── ANTHROPIC_API_KEY 必須
export ANTHROPIC_API_KEY=sk-ant-...
harness run --model claude-haiku-4-5
```

## 期待結果

- tool_calls=2 (report_evidence + request_transition)
- cost ~$0.008 (haiku-4-5)
- workflow 終了 (next=[])

## 動作確認済

Step 1 dogfood (2026-05-18、 `C:	mp\dogfood-v2-*`) で haiku-4-5 完走、 cost $0.0079 実測。
