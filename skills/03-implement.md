# フェーズ 3: implement

## ゴール
plan に従って実装する。各ソースファイルは ≤200 行。

## やること
1. plan の変更ファイル一覧どおりに実装する。
2. 作った/変えたソースファイルを 1 つずつ登録する。name は `impl:<短い識別子>`:
   ```
   harness record-artifact impl:state src/state.rs
   harness record-artifact impl:cli   src/main.rs
   ```
   複数登録してよい。
3. 各 `impl:` ファイルが 200 行以下であること。超えたら責務分割。
4. 成果物に禁止語を残さない。
5. 終わったら `harness advance`。却下されたら理由（impl_artifacts_exist / impl_artifacts_size_ok / no_forbidden_words）を見て直す。

## 禁止
- フェーズスキップ。
- `state/*.jsonl` の直接編集。
- 成果物に禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー, 仮置き）を残すこと。
