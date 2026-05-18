# skill: security

このノードのゴール: 実装変更がセキュリティ的リスクを持たないか checklist で確認し、
`security_review` evidence を提出する。secret scan / dep audit を含む。

## 前提

- test phase が緑（cmd_exit_0 で test スイート pass 済）

## 順序

1. **保留 gate を確認**
   ```
   harness status
   ```
   配線されている security scan cmd（`gitleaks detect --no-git --redact` 等、detect が
   見つけたもの。未検出なら `false # configure security-scan ...`）を確認。

2. **diff を抽出** ── 何が変わったか:
   ```
   git diff main..HEAD --stat
   git diff main..HEAD -- 'src/' | head -200
   ```

3. **セキュリティ checklist**

   - [ ] 認証/認可 path の変更が intentional + reviewed か
   - [ ] secret / credentials / API key の hard-code なし
   - [ ] SQL / command / path-traversal injection の risk なし
   - [ ] log に sensitive data（token / PII / 内部 URL）が出ない
   - [ ] dependency 追加が intentional、license / supply chain 確認済
   - [ ] file I/O の path validation あり（`../` の guard）

4. **secret scan を走らせる** ── workflow.toml の exit_gate で配線済:
   ```
   gitleaks detect --no-git --redact
   ```
   exit 0 で `cmd_exit_0` gate が pass。検出があれば redact / 削除して再実行。

5. **dependency audit（任意・推奨）**
   ```
   cargo audit                       # Rust
   pnpm audit --prod                 # Node.js
   pip-audit                         # Python
   ```
   既知 CVE があれば判断（critical/high は基本 reject）。

6. **判定 evidence を提出** ── exit_gate `evidence_recorded { key = "security_review" }`
   が pass するように:
   ```
   harness report-evidence security_review '{"verdict":"approved","risk_items":[],"notes":"..."}'
   ```
   risk が残るなら `verdict: "rejected"` で `harness back "security risk: ..."`。

7. **進める**:
   ```
   harness request-transition review
   ```

## 完了条件（exit_gates）

このノードの出口 gate（`workflow.toml` の `[[node]] id = "security"` を参照）:

- `cmd_exit_0 { cmd = "<security-scan>" }` ── secret scan が exit 0
  （未検出なら `false # configure security-scan ...` ── gitleaks インストール推奨）
- `evidence_recorded { key = "security_review" }` ── 上の `report-evidence` で pass

## 詰まったとき

- gitleaks が誤検出 → false positive を redact ＋ notes に rationale を書く
- dep の CVE が修正不能 → `harness ask` で許容判断、または `harness back "dep 入替"`
- これ以上進めない → `harness stuck "<理由>"`

## 禁止

- 「change は小さいから skip」で checklist を埋めないこと（最低限の確認は必須）
- risk_items を空にしたいだけで rejected を approved に書き換えること（gate 偽装）
- secret を redact せず log / artifact に残すこと
- 状態ファイル（イベントログ）の直接編集
- 禁止語（TODO, TBD, WIP, FIXME, 未定, 未確定, 要検討, 検討中, 対応予定, サンプル, ダミー,
  仮置き）を成果物に残すこと
- `report-evidence` の `gate` 引数に gate プリミティブ種別名（`evidence_recorded` 等）を
  渡すこと ── 渡すのは evidence の **key 名**（`security_review`）
