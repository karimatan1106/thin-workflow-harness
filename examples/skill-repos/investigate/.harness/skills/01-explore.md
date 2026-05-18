# Skill: explore

repository の構造を調査して symbol を特定する。 結果を `exploration` artifact として登録する。

## 前提

- `harness-lspd` (CKG primitive CLI) が PATH に install 済。 未 install なら:
  ```bash
  cargo install --path /path/to/thin-workflow-harness/examples/skill-tools-archive/lsp-daemon
  ```
- cwd が Rust workspace (Cargo.toml が直下にある) を想定。 他言語でも `--lang` 指定で動く。

## 手順

1. **outline 取得**:
   ```
   run_command("harness-lspd outline src/lib.rs")
   ```
   トップレベル symbol (fn / struct / enum / mod) の一覧が返る。

2. **symbol 検索** (任意):
   ```
   run_command("harness-lspd find-symbol main --lang rust --root .")
   ```
   workspace 全体から `main` symbol の位置を引く。 daemon (`harness-lspd lsp-daemon serve`) が起動済なら高速。

3. **結果の整理**:
   - `edit_file` で `exploration-summary.md` を作り outline + find-symbol の結果を貼る (≤200 行)。
   - `record_artifact("exploration_summary", "exploration-summary.md")` で artifact 登録。

4. **遷移**:
   - exit_gates の `artifact_registered { name_or_prefix = "exploration" }` が pass する。
   - `request_transition` を呼ぶ ── 次 node `report` へ。

## 詰まったら

- `harness-lspd` が無い・ workspace が Rust でない・ outline が空: `ask` で人間に確認するか `stuck "<理由>"` でエスカレ。
- 「決定」を訊け、 「情報」を訊くな ── `find-symbol` 結果は自分で読み解け。
