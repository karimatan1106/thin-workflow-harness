> DESIGN.md の補助。設計の方針であって最終確定ではない部分も含む。

# host-capabilities — 能力と harness の分離、Phase 0 ↔ 1

harness は常に sequencer + gater + 状態機械。plan / security review / code review / 人間への質問 / sub-worker / 編集境界の強制 — これらの「能力」は host が提供する（Phase 0 では Claude Code の組込み、Phase 1 では harness の等価物）。harness はそれらを再実装せず*参照*し、組込みの*手順*を `skills/` に移植する — 「thin harness, fat skills」そのもの。

## 1. 能力 ↔ host の対応表

| 能力 | Phase 0（agent = Claude Code、`harness` コマンドを叩く） | Phase 1+（agent = harness runtime が生 API で spawn） |
|---|---|---|
| 徹底的な計画 | plan モード（read-only research を強制 → ExitPlanMode で計画提示）— plan ノードがこれに対応 | plan ノードの skill が同じ規律を複製（semantic クエリで read-only 探索 → 詳細な plan artifact） |
| セキュリティレビュー | `/security-review` skill（多段解析）— security ノードが invoke | security ノードの skill = `/security-review` の手順を移植（`skills/security-review.md`）＋ `cmd_exit_0` スキャナ |
| コードレビュー | `/review` skill — review ノードが invoke | review ノードの skill = `/review` の手順を移植（`skills/code-review.md`）＋ spec チェックリスト自己レビュー |
| 人間への質問 | `harness ask` が文字通り AskUserQuestion を使う（構造化質問 ＋ 選択肢） | `harness ask` が質問キューに書く → 人間が `harness answer`（または UI） |
| sub-worker | L3 sub-worker = Agent/Task ツール | L3 sub-worker = 生 API |
| 編集境界の強制 | Claude Code hook（PreToolUse で blast radius 外編集を block — ボーナス enforcement 層、`harness init` がオプションでスキャフォールド） | runtime の tool-call インターセプタ |

## 2. 設計上の含意

- harness の各ノードは「能力」に対応。Phase 0 ではその能力を Claude Code の組込みが満たす。Phase 1 では harness が等価物を提供。harness 自身は plan モードも `/security-review` も再実装しない — *参照*して*手順を移植*するだけ。これが最大限「thin」。
- `skills/` には各能力の*ポータブルな*版（手順）が入る — Phase 1（Claude Code 無し）に substance を提供。ノードの skill は「host に組込みがあればそれを優先、なければこの手順に従え」と書く（または `host` 設定で分岐）。
- 「harness は hook システムを持たない」（DESIGN）と矛盾しない — harness *自身*は hook を持たない、host の hook を*活用*するだけ（Phase 0 のボーナス）。

## 3. `host` 設定

`.harness/config`（または `workflow.toml` の `[meta]`）に `host = "claude-code" | "runtime"`（| 他のエージェントホスト）。

- `claude-code` なら plan モード・`/review`・`/security-review`・AskUserQuestion・hook・Agent ツールが使える。
- `runtime` なら harness の等価物。
- ノード skill が `{{#if host == "claude-code"}}...{{else}}...{{/if}}` 的に分岐（または skill が常に手順を持ち「host の組込みがあれば優先」と書くだけでもよい — 要確認、実装で確定）。

## 4. Phase 0 が借りるもの / Phase 1 が複製するもの（まとめ）

- Phase 0 は Claude Code から: plan モード、`/review`、`/security-review`、AskUserQuestion、Agent ツール、hook。
- Phase 1 は複製: plan ノード skill（plan モードの規律）、`skills/code-review.md`（`/review` の手順）、`skills/security-review.md`（`/security-review` の手順）、質問キュー ＋ `harness answer`（AskUserQuestion の代替）、生 API sub-worker（Agent ツールの代替）、tool-call インターセプタ（hook の代替）。
