//! `docs/architecture/` と `docs/adr/` の skeleton (arc42 6 セクション + ADR INDEX)。
//!
//! 設計指針:
//! - `docs/` 全体を **Open Knowledge Format (OKF) v0.1 準拠の知識バンドル**として生成する
//!   (Google Cloud, 2026-06-12)。非予約 .md は YAML frontmatter + **非空 `type`(OKF 必須)** を持つ。
//!   既存の doc-id / status / supersedes / tags / description / last-reviewed は OKF の
//!   「未知キーは round-trip 保持」に従い拡張キーとして温存 ── agent が body を読まずに relevance 判定可能に。
//! - OKF 予約ファイルを bundle root(`docs/`)に置く: `index.md`(frontmatter 無・漸進的開示の目次)、
//!   `log.md`(ISO 8601 日付でグループした変更履歴)。概念 ID = バンドル内パスから `.md` を除いたもの。
//! - 適合チェックは fail-safe な `bin/okf_check.mjs`(docs/ 不在→N/A・既定 非ブロッキング・OKF_STRICT=1 で強制)。
//! - arc42 12 セクションから 6 (Context / Building Blocks / Runtime / Decisions /
//!   Quality / Risks) に収束させる現実的運用。
//! - 各ファイル ≤200 行を強制 (review phase の max_lines gate)。
//! - `architecture/` は mutable snapshot、 `adr/` は immutable append-only log で分離。
//! - 既存ファイルは絶対上書きしない (ユーザー設計書を保護)。

use std::path::Path;

/// `(相対パス, 本文)` の skeleton ファイル一覧。
pub(super) fn entries() -> &'static [(&'static str, &'static str)] {
    &[
        // OKF v0.1 予約ファイル (bundle root = docs/)。index.md は frontmatter を持たない。
        ("docs/index.md", OKF_INDEX_MD),
        ("docs/log.md", OKF_LOG_MD),
        ("docs/architecture/README.md", README_MD),
        ("docs/architecture/01-context.md", CONTEXT_MD),
        ("docs/architecture/02-blocks.md", BLOCKS_MD),
        ("docs/architecture/03-runtime.md", RUNTIME_MD),
        ("docs/architecture/04-decisions.md", DECISIONS_MD),
        ("docs/architecture/05-quality.md", QUALITY_MD),
        ("docs/architecture/06-risks.md", RISKS_MD),
        ("docs/adr/INDEX.md", ADR_INDEX_MD),
    ]
}

/// `repo_root` 配下に skeleton を生成。 **既存ファイルは skip**。
/// 戻り値: 生成したファイル一覧。
pub(super) fn write_skeleton(repo_root: &Path) -> Result<Vec<String>, String> {
    let mut created = Vec::new();
    for (rel, body) in entries() {
        let path = repo_root.join(rel);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, body).map_err(|e| format!("{}: {e}", path.display()))?;
        created.push(rel.to_string());
    }
    Ok(created)
}

const README_MD: &str = r#"---
type: architecture-index
doc-id: arch-readme
status: current
supersedes: []
tags: [architecture, index, arc42]
description: マスター設計書の索引。 arc42 6 セクション + modules への link 表。
last-reviewed: TBD
---

# Architecture Master Index

このリポジトリのマスター設計書 (mutable snapshot)。 `docs/adr/` (immutable log)
と対になる。 1 ファイル ≤200 行、 YAML frontmatter 必須、 review phase で更新。

## arc42 セクション

| § | ファイル | 内容 |
|---|---|---|
| 1 | [01-context.md](01-context.md) | scope / actors / 外部 IF |
| 2 | [02-blocks.md](02-blocks.md) | module 構成 + Mermaid C4 container 図 |
| 3 | [03-runtime.md](03-runtime.md) | 主要 scenario のデータフロー |
| 4 | [04-decisions.md](04-decisions.md) | ADR への link 表 (本文は `docs/adr/`) |
| 5 | [05-quality.md](05-quality.md) | 品質目標 / SLO / 不変条件 |
| 6 | [06-risks.md](06-risks.md) | trade-off / 技術的負債 |

## modules/

大きな module はサブディレクトリで階層化。 各 module は責務・入出力・不変条件・
関連 ADR を持つ独立した ≤200 行ドキュメント。

```
modules/
└── <module-name>.md   # 1 module = 1 ファイル (大きければサブディレクトリ)
```

## 更新ルール

- **research phase**: このファイルと該当 arc42 セクションを pinpoint で読む。
- **review phase**: 構造変更があれば該当 section を更新し `last-reviewed` を
  YYYY-MM-DD に書き換える。
- 新規 Why が生じた場合は `docs/adr/` に ADR を起票し、 `04-decisions.md` の
  link 表に append。
- 古くなった section には inline で `[STALE: see ADR-NNN]` を残す (削除しない)。
"#;

const CONTEXT_MD: &str = r#"---
type: architecture-section
doc-id: arch-01-context
status: current
supersedes: []
tags: [architecture, context, actors]
description: システムの scope / 外部 actors / 主要 IF を俯瞰する arc42 §1。
last-reviewed: TBD
---

# 1. Context & Scope

## 1.1 目的

このシステムが**何を解決するか**を 1 段落で。 ビジネスゴール、 主要 use case。

## 1.2 Scope

- **対象**: <この設計書がカバーするシステム境界>
- **対象外**: <意図的に除外する範囲>

## 1.3 主要 Actors

| Actor | 種別 | 関与する use case |
|---|---|---|
| (例) End User | 人間 | 〜〜 |
| (例) 外部 API | システム | 〜〜 |

## 1.4 外部 Interface

| 相手 | プロトコル | 方向 | 備考 |
|---|---|---|---|
| (例) 外部 DB | PostgreSQL | bidir | 〜〜 |

## 1.5 主要 Constraint

- (例) 法的: 〜〜
- (例) 技術的: 〜〜
- (例) 運用: 〜〜
"#;

const BLOCKS_MD: &str = r#"---
type: architecture-section
doc-id: arch-02-blocks
status: current
supersedes: []
tags: [architecture, modules, c4-container]
description: 全 module の責務・依存関係・C4 container 図 (arc42 §2)。
last-reviewed: TBD
---

# 2. Building Block View

## 2.1 C4 Container Diagram

```mermaid
C4Container
    title System Containers
    Person(user, "User", "Description")
    System_Boundary(sys, "System") {
        Container(api, "API", "Tech", "Description")
        ContainerDb(db, "Database", "Tech", "Description")
    }
    Rel(user, api, "Uses", "HTTPS")
    Rel(api, db, "Reads/Writes")
```

> **重要**: code 変更で module 構成が変わったら、 この図も同期更新する
> (review phase の必須項目)。

## 2.2 Module 一覧

| Module | 責務 (1 行) | 詳細 |
|---|---|---|
| (例) `<name>` | 〜〜 | [modules/<name>.md](modules/<name>.md) |

## 2.3 依存方向の原則

- (例) 上位層は下位層に依存してよいが、 逆は禁止
- (例) cross-module は API を経由、 内部実装に依存しない

## 2.4 関連 ADR

- ADR-NNN: <主要なアーキテクチャ決定への link>
"#;

const RUNTIME_MD: &str = r#"---
type: architecture-section
doc-id: arch-03-runtime
status: current
supersedes: []
tags: [architecture, runtime, dataflow]
description: 主要 scenario のデータフロー / sequence (arc42 §3)。
last-reviewed: TBD
---

# 3. Runtime View

## 3.1 主要 Scenario 一覧

| # | Scenario | 関与 module |
|---|---|---|
| S-1 | (例) ユーザーログイン | api, auth, db |
| S-2 | (例) データ取得 | api, cache, db |

## 3.2 Scenario S-1: <名前>

```mermaid
sequenceDiagram
    participant U as User
    participant A as API
    participant D as DB
    U->>A: Request
    A->>D: Query
    D-->>A: Result
    A-->>U: Response
```

**注釈**:
- (例) 失敗時の rollback は 〜〜
- (例) timeout は 〜〜秒

## 3.3 Scenario S-2: <名前>

(同フォーマットで追記)

## 3.4 関連 ADR

- ADR-NNN: <データフロー関連の決定への link>
"#;

const DECISIONS_MD: &str = r#"---
type: architecture-section
doc-id: arch-04-decisions
status: current
supersedes: []
tags: [architecture, adr-index, decisions]
description: ADR への link 表のみ。 本文は docs/adr/ 配下に置く (arc42 §4)。
last-reviewed: TBD
---

# 4. Architecture Decisions

このファイルは **link 表のみ**。 各 ADR の本文は [`docs/adr/`](../adr/) を見ること。
ADR は immutable (1 ADR = 1 Why、 append-only)。

詳細な一覧は [`docs/adr/INDEX.md`](../adr/INDEX.md)。

## 主要 ADR (highlight)

review phase で「特に押さえるべき決定」を以下に link する。 全 ADR は INDEX 参照。

| ADR | Title | Status | 関連 section |
|---|---|---|---|
| (例) ADR-001 | 〜〜 | Accepted | [02-blocks.md](02-blocks.md) |

## 運用ルール

- 新規 ADR を起票したら、 上の表に 1 行追加 (review phase で実施)
- supersede されたら status を Superseded に更新
- 古い ADR は削除せず、 INDEX で履歴を辿れる状態を維持
"#;

const QUALITY_MD: &str = r#"---
type: architecture-section
doc-id: arch-05-quality
status: current
supersedes: []
tags: [architecture, quality, slo, invariants]
description: 品質目標 / SLO / 不変条件 (arc42 §5)。
last-reviewed: TBD
---

# 5. Quality Goals & Invariants

## 5.1 品質目標 (優先順)

| # | 目標 | 測定指標 |
|---|---|---|
| Q-1 | (例) 可用性 99.9% | 月次 uptime |
| Q-2 | (例) p95 latency < 200ms | API metrics |

## 5.2 SLO

| SLO | 目標値 | 測定窓 |
|---|---|---|
| (例) Error rate | < 0.1% | 7d rolling |

## 5.3 全体 Invariants

各 module に固有の不変条件は `modules/<name>.md` に書く。 ここはシステム全体に
跨るもののみ:

- **INV-1**: (例) 認証されていないユーザーは保護リソースにアクセス不可
- **INV-2**: (例) すべての書き込みは audit log に記録

## 5.4 関連 ADR

- ADR-NNN: <品質目標関連の決定>
"#;

const RISKS_MD: &str = r#"---
type: architecture-section
doc-id: arch-06-risks
status: current
supersedes: []
tags: [architecture, risks, tradeoffs, tech-debt]
description: 既知の trade-off / 技術的負債 / 残存リスク (arc42 §6)。
last-reviewed: TBD
---

# 6. Risks & Technical Debt

## 6.1 既知の Trade-off

| # | Trade-off | 採用理由 | 影響範囲 |
|---|---|---|---|
| T-1 | (例) eventual consistency 採用 | latency 要件優先 | 集計 view |

## 6.2 技術的負債

| # | 内容 | 影響 | 解消予定 |
|---|---|---|---|
| D-1 | (例) `<deprecated lib>` 残置 | security patch 不可 | ADR-NNN で計画 |

## 6.3 残存リスク

| # | リスク | 発生確率 | 影響 | 緩和策 |
|---|---|---|---|---|
| R-1 | (例) 〜〜 | 中 | 高 | 〜〜 |

## 6.4 関連 ADR

- ADR-NNN: <リスク対応の決定>
"#;

const ADR_INDEX_MD: &str = r#"---
type: adr-index
doc-id: adr-index
status: current
supersedes: []
tags: [adr, index, decisions]
description: 全 ADR の一覧 + status table。 append-only。
last-reviewed: TBD
---

# ADR Index

このファイルは **append-only** (削除・並び替え禁止)。 各 ADR の本文は
`ADR-NNN-<slug>.md` を参照。

## ADR 一覧

| ID | Title | Status | Date | Supersedes |
|---|---|---|---|---|
| (例) ADR-001 | 〜〜 | Accepted | YYYY-MM-DD | — |

## Status 凡例

- **Proposed**: 提案中、 まだ accept されていない
- **Accepted**: 採用、 現在有効
- **Superseded**: 別 ADR に置き換えられた (本文は immutable のまま残す)
- **Deprecated**: 採用したが廃止予定

## 新規 ADR 起票手順 (review phase)

1. 次番号 NNN を採番 (INDEX 末尾 +1)
2. `ADR-NNN-<slug>.md` を作成 (slug は kebab-case)
3. frontmatter で `type: adr`(OKF 必須) / `status: Accepted` / `supersedes: []` / `superseded-by: null`
4. 本文 5 セクション: Context / Decision / Consequences / Review Trigger / Related
5. このファイルに 1 行 append
6. 既存 ADR を覆す場合: 新 ADR の `supersedes: [ADR-XXX]`、 旧 ADR の
   `superseded-by: ADR-NNN` (旧 ADR の `status: Superseded` に変更、 本文は不変)
"#;

/// OKF v0.1 予約ファイル `docs/index.md`。**frontmatter を持たない**(spec §6)。
/// バンドルの漸進的開示エントリ。リンクは GitHub でも描画される相対形で書き、
/// OKF 推奨の bundle 相対絶対 `/...` 形は注記に留める(両形とも valid)。
const OKF_INDEX_MD: &str = r#"# Knowledge Bundle (OKF v0.1)

この `docs/` は **Open Knowledge Format (OKF) v0.1** 準拠の知識バンドル
(<https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf>)。
非予約 `.md` は YAML frontmatter + 非空 `type` を持ち、 markdown リンクで相互接続して
グラフを成す。 `index.md`(本ファイル・frontmatter 無)と `log.md`(変更履歴)は OKF 予約ファイル。

## Architecture (arc42 マスター設計書 ── mutable snapshot)

- [architecture/README.md](architecture/README.md) ── 索引
- [architecture/01-context.md](architecture/01-context.md) ── scope / actors / 外部 IF
- [architecture/02-blocks.md](architecture/02-blocks.md) ── module 構成 / C4 container 図
- [architecture/03-runtime.md](architecture/03-runtime.md) ── 主要 scenario のデータフロー
- [architecture/04-decisions.md](architecture/04-decisions.md) ── ADR への link 表
- [architecture/05-quality.md](architecture/05-quality.md) ── 品質目標 / SLO / 不変条件
- [architecture/06-risks.md](architecture/06-risks.md) ── trade-off / 技術的負債

## Decisions (ADR ── immutable append-only log)

- [adr/INDEX.md](adr/INDEX.md) ── 全 ADR 一覧 + status

## 規約 (OKF v0.1)

- **概念 ID** = バンドル内パスから `.md` を除いたもの (例 `architecture/01-context`)。
- **必須 frontmatter** = 非空 `type`。 推奨 = `title` / `description` / `resource` / `tags` / `timestamp`。
  既存の `doc-id` / `status` / `last-reviewed` 等は拡張キーとして保持される。
- **リンク** = 有向・型なしの関係エッジ。 安定性重視なら bundle 相対の絶対形 `/architecture/01-context.md` を推奨
  (移動に強い)。 consumer は壊れリンクを許容する。
- **適合チェック** = `node .harness/bin/okf_check.mjs`(fail-safe・既定 非ブロッキング・`OKF_STRICT=1` で強制)。
"#;

/// OKF v0.1 予約ファイル `docs/log.md`(spec §7)。 変更を ISO 8601 日付でグループし、 新しい日付を上に append。
/// docdesign / review phase が知識バンドル更新時に追記する。 frontmatter は持たない。
const OKF_LOG_MD: &str = r#"# Log

OKF v0.1 予約ファイル。 `docs/` 知識バンドルの意味のある変更を **ISO 8601 日付でグループ**して記録する
(新しい日付を上に append・削除しない)。 docdesign / review phase が更新時に追記する。

## YYYY-MM-DD
- (例) `harness init` で OKF v0.1 知識バンドル(arc42 + ADR)を初期化。
"#;
