# Claude Code PreToolUse hooks for thin-workflow-harness

このディレクトリの 3 つの hook は **harness binary の外側** で動く
opt-in な安全弁です。harness の L1-L4 gate は事後検査なので、agent が
phase 中にやってはいけない操作を実行してしまうのを未然に止められません。
これらの hook を Claude Code の `PreToolUse` layer に登録すると、
ツール呼び出しが実行される前に block できます。

## thin philosophy との関係

harness binary は機能を持ちすぎないことを是としています。よってこの
ような Claude 固有の hook は **binary に同梱しない** 方針です。代わりに
`~/.claude/hooks/` に置き、ユーザの Claude Code 設定で配線します。
ハーネス側はあくまで run の現在状態 (`harness status`) を提供するだけです。

## 3 つの hook

| Hook | 対象 tool | 動作 | 失敗時 exit | bypass env |
|---|---|---|---|---|
| `phase-edit-guard.py` | Edit / Write / MultiEdit | `harness status` の現在 phase が research / hearing / design のとき、 `.rs / .py / .ts / .js / .go / .rb / .java` への編集を block | 2 | `SKIP_PHASE_GUARD=1` |
| `forbidden-word-guard.py` | Edit / Write / MultiEdit | `new_string` / `content` / `edits[].new_string` に TODO/TBD/WIP/FIXME/XXX/HACK/未定/未確定/要検討/検討中/対応予定/サンプル/ダミー/仮置き が含まれていたら block | 2 | `SKIP_FORBIDDEN_WORDS=1` |
| `loop-detector.py` | すべて | `(tool_name, sha256(tool_input))` を `.harness/loop-state.json` に append し、 末尾 3 件が同一なら block | 2 | `SKIP_LOOP_DETECTION=1` |

いずれも `.harness/` または `harness` binary が存在しない環境では silent
に exit 0 (pass-through) する設計です。最悪でも何も止めません。

## install (Windows PowerShell)

```powershell
Copy-Item examples\hooks\*.py $env:USERPROFILE\.claude\hooks```

POSIX 環境では `cp examples/hooks/*.py ~/.claude/hooks/` で同様。

## `~/.claude/settings.json` への配線例

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write|MultiEdit",
        "hooks": [
          { "type": "command", "command": "python ~/.claude/hooks/phase-edit-guard.py" },
          { "type": "command", "command": "python ~/.claude/hooks/forbidden-word-guard.py" }
        ]
      },
      {
        "matcher": ".*",
        "hooks": [
          { "type": "command", "command": "python ~/.claude/hooks/loop-detector.py" }
        ]
      }
    ]
  }
}
```

Windows でユーザ home の展開が効かない場合は絶対 path に置き換えて
ください。

```json
"command": "python C:/Users/owner/.claude/hooks/phase-edit-guard.py"
```

## bypass

緊急時は環境変数で個別に無効化できます。Claude Code を起動した shell で:

```powershell
$env:SKIP_PHASE_GUARD = "1"
$env:SKIP_FORBIDDEN_WORDS = "1"
$env:SKIP_LOOP_DETECTION = "1"
```

bypass は **監査証跡には残らない** ので、使った事実は人間側で記録する
こと。

## stdin / exit code 仕様

Claude Code は PreToolUse hook に次の JSON を stdin で渡します:

```json
{
  "tool_name": "Edit",
  "tool_input": { "file_path": "src/foo.rs", "old_string": "...", "new_string": "..." }
}
```

hook の終了コードに応じて Claude Code は:

* `0` → tool 実行を許可
* `2` → tool 実行を block し stderr を model に提示
* それ以外 → warning は出すが block しない

このリポジトリの hook は block 時に必ず exit 2 + stderr に理由を出力
します。

## phase 取得の仕組み (phase-edit-guard)

1. `HARNESS_HOME` を見る。設定済みならその直下の `.harness/` を起点。
2. なければ cwd を root に向けて遡り `.harness/` を探す。
3. `harness status` を実行し `node   : N/M <id>` 行から id を抽出。
4. id (`research` / `hearing` / `design`) と一致したら guard 発動。

`.harness/` が見つからない / `harness` が PATH にない / status が失敗
する場合はすべて pass-through (exit 0) です。

## 動作確認

```powershell
$json = '{"tool_name":"Edit","tool_input":{"file_path":"foo.rs","new_string":"// TODO impl"}}'
$json | python examples\hooks\forbidden-word-guard.py
echo $LASTEXITCODE  # 2 になれば OK
```

3 つすべての dry-run 確認は同様に stdin で JSON を流すだけです。

## 既知の限界

* `phase-edit-guard` は `harness status` をサブプロセスで呼ぶので
  ~50-150ms overhead が乗ります。大量編集セッションでは体感できる
  かもしれません。気になる場合は `SKIP_PHASE_GUARD=1` で off に
  できます (ただし audit を壊さないでください)。
* `forbidden-word-guard` は AST を見ないので docstring 内の "TODO" など
  本文中の文字列にも反応します。逃したい場合は bypass を使うか、
  文言を変える方が早いです。
* `loop-detector` は 1 セッション内のみの記録です。harness run を
  またぐ繰り返しは検出しません。
