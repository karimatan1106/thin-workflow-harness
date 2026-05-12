# フェーズ 4: test

## ゴール
実装が動くことをテストで確認し、結果を記録する。

## やること
1. テストを実行する（ユニットテスト、スモークテスト、ビルド確認など計画したもの）。
2. 実行コマンドと終了コードを gate evidence として報告する:
   ```
   harness report-gate test_result '{"command":"cargo test","exit_code":0}'
   ```
   `exit_code` が 0 でないと `harness advance` は通らない。
3. テストが落ちていたら implement に戻る:
   ```
   harness back "test 失敗: <要点>"
   ```
4. 通ったら `harness advance`。

## 禁止
- フェーズスキップ。
- `state/*.jsonl` の直接編集。
- 落ちているのに `exit_code: 0` と偽って報告すること。
