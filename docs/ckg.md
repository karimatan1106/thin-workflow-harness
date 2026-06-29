---
type: reference
title: "CKG — コードナレッジグラフ詳細設計"
description: "> DESIGN.md §9（context 圧縮戦略）の詳細。索引器・グラフは外部/プラガブル（harness 本体に索引器は内蔵しない）。実装は Phase 1.5（CLI コアの後、ランタイムと並行可）。"
tags: [harness, docs]
---

# CKG — コードナレッジグラフ詳細設計

> `DESIGN.md` §9（context 圧縮戦略）の詳細。索引器・グラフは外部/プラガブル（harness 本体に索引器は内蔵しない）。実装は Phase 1.5（CLI コアの後、ランタイムと並行可）。
>
> **注記**: 以下のスキーマ・粒度・ストレージ形式は設計の方針であって最終確定ではない（実コードを書く際に詰める）。確定しているのは §6 のクエリ API 表面（コマンド名と各々の意味 ── DESIGN / schemas からも参照される正典）であって、その裏のスキーマ・テーブル定義・索引粒度は仮。

## 1. 目的

コードを読まずに構造的な問い（定義は？呼んでるのは？型の中身は？実装してるのは？依存は？影響範囲は？カバーするテストは？）に答える queryable な artifact。

返るのは位置（`file:line`, シンボル種別）であって本体ではない ── エージェントは本体を読むのを実際に触る数シンボルだけに絞れる。blast radius 特定と `traceability_closed` gate に効く。

## 2. スキーマ（ノード / エッジ）

### ノード種別

- `file`
- `module`（namespace / package 含む）
- `symbol`（function, method, type（struct / enum / class）, trait / interface, const / static, macro）

### ノードのフィールド

- `id` — 安定識別子。理想は SCIP の symbol moniker、または `<file>#<qualified_name>`
- `kind`
- `name`
- `qualified_name`
- `file`
- `start_line`
- `end_line`
- `lang`
- `signature`（symbol のみ）
- `doc` — docstring（先頭1行 or 全文）
- `tags` — `test` / `public` / `generated` 等

### エッジ種別

- `contains` — file→symbol, module→symbol
- `references` — symbol→symbol（使用箇所）
- `calls` — function→function（references の呼び出し意味のサブセット）
- `imports` — file/module→module
- `implements` — type→trait
- `inherits` — type→type
- `has_type` — 変数/パラメータ symbol→type
- `tested_by` — symbol→test-symbol（後述ヒューリスティック or coverage マップ由来）

### エッジのフィールド

- `from_id`
- `to_id`
- `kind`
- `file`
- `line`（エッジ発生箇所）

### 派生（保存せずクエリ時に計算）

- `closure(sym, depth)` = calls / references の BFS
- `impacted_by(sym)` = 逆 references 閉包
- `tested_by(sym)` = 前方 tested_by エッジ
- `deps(module)` = imports 前方

## 3. 構築（方針 ── 索引器の選定・段階付けも実装で確定）

**スクラッチで意味解析エンジンは書かない。既存の索引器 / パーサ / Language Server の出力を取り込むだけ。** harness 本体に索引器は内蔵しない（プラガブル、§冒頭注記）。

### 一次: SCIP 索引器

**SCIP = Sourcegraph の Code Intelligence Protocol**（LSIF の後継でよりシンプル / 効率的、protobuf で「定義・参照・実装・docstring・symbol moniker」を記述）。SCIP プロジェクト（github.com/sourcegraph/scip）が proto スキーマ＋`.scip` を読む CLI / ライブラリを提供する。

実在する言語別の SCIP 索引器（CKG はこれにシェルアウトし、出力 `.scip` を CKG ストアに fold する）:

- **Rust** ── `rust-analyzer scip`（rust-analyzer に組み込み、高品質）
- **TypeScript / JavaScript** ── `scip-typescript`（Sourcegraph）
- **Python** ── `scip-python`（Pyright ベース）
- **Java / Kotlin / Scala** ── `scip-java`
- **Go** ── `scip-go`
- **C / C++** ── `scip-clang`
- **Ruby** ── `scip-ruby`

Rust 側では `scip` クレート（or `.proto` から protobuf 型を生成）で `.scip` をパース → SQLite に fold する。SCIP がノード / エッジデータの大半（定義・参照・実装・docstring・moniker）を供給。

**限界**: SCIP 索引器は対象プロジェクトが*ビルド / 型チェックできる*必要がある（ビルド依存・ツールチェーン・設定が揃っていないと索引できない、または部分的にしか索引できない）。CI に索引ステップを置く運用が前提（→ §9 限界に再掲）。

### フォールバック（SCIP 索引器が無い言語）

優先順:

1. **`tree-sitter`（github.com/tree-sitter）＋ `tree-sitter-stack-graphs`** ── tree-sitter はほぼ全言語のパーサ（構文構造＝関数・型・range・シグネチャを出す。`outline` に最適、ただし参照は解決しない・ビルド不要）。`tree-sitter-stack-graphs`（GitHub のコードナビが使う ── tree-sitter パース＋言語ごとの stack graph ルールから名前解決グラフを作る ＝ 参照も解決、言語横断的）。型対応の per-language SCIP 索引器より精度は落ちるが、ずっと汎用（言語ごとに索引器を用意しなくてよい）。
2. **`tree-sitter`（outline）＋ LSP on-demand（参照、下記）** ── stack-graphs を使わず、cross-reference は毎回 LSP に訊く。
3. 最低限のフォールバック ── `universal-ctags`（定義は出るが参照は出ない。CKG には粗すぎる、`find-symbol` / `outline` の代用程度）。

**`outline` は常に tree-sitter で出す**（全言語、安い、ビルド不要 ── SCIP も stack-graphs も要らない）。cross-reference（`refs` / `callers` / `closure` / `impacted-by`）は SCIP か stack-graphs か LSP が要る。

### LSP on-demand 経路（precompute ゼロのバックエンド）

任意の Language Server（rust-analyzer / tsserver / pyright / gopls / clangd / jdtls …）は `textDocument/definition`・`references`・`documentSymbol`・`callHierarchy`・`typeHierarchy`・`implementation` に答える ── **グラフを*作らず*、毎回 LSP に訊く**。Rust から LSP を駆動するクレート: `lsp-types`（プロトコル型）／ `lsp-server`（JSON-RPC トランスポート）／ `async-lsp`（高レベルクライアント）。

**この harness が動く環境（Crypto プロジェクト）は既に Serena（LSP を agent ツールにラップしたもの）を使っている**（`.claude/rules/code-search-policy.md`）── なので最速で動く CKG バックエンドは「**Serena / LSP ブリッジ**」（precompute 無し・ライブクエリ）。`closure --depth N` は安くできない（`references` を実質 N 回叩く）が、precompute ゼロ・任意の LSP 対応言語で動く。

### test 検出（tested_by エッジ）

ヒューリスティック ── プロジェクトのテスト glob（`**/tests/**`, `*_test.rs`, `test_*.py` 等、config が宣言）にマッチするファイル内のシンボルが非テストシンボルを参照 → tested_by エッジ。

より精密にしたければ、テストランナーが coverage→symbol マップを出せるならそれを優先。config がテスト glob と任意で coverage-map コマンドを宣言。

### 多言語 repo

複数 `.scip` を1つのストアにマージ ── moniker が言語 / パッケージで名前空間化されてるので衝突しない。

## 4. インクリメンタル更新の粒度（方針: ファイル単位 ＋ 逆依存閉包 ＋ フルフォールバック ── 実装で確定）

- 再索引の単位は**ファイル**（索引器はファイル or コンパイル単位で動く、単一シンボルでは動かない ── シンボル単位増分はやらない、複雑さに見合わない）。
- `harness reindex`: git に変更ファイルを聞く（`git diff --name-only <last-indexed-rev> HEAD`）→ 変更ファイル**＋その逆依存閉包**（変更ファイル内のシンボルを参照する他ファイル ── CKG の impacted-by で求まる。理由: file A が file B のシンボル S を参照していて B が変わって S が動いたら、A は変わってなくても A→S のエッジを更新する必要がある）を再索引 → CKG ストアにパッチ（変更 / 逆依存ファイルのノード・エッジを削除して再投入）。巨大 repo でもフル再索引よりずっと安い。
- 逆依存閉包自体が大きい場合（広く使われるユーティリティ ＝ "hot symbol"）はフル再索引にフォールバック。索引器が増分非対応ならフル。
- stale 判定: `meta.index_rev` ≠ git HEAD なら一部ファイルが stale → そのファイルへのクエリは `stale: re-index 推奨` フラグ付きで返す（黙って古いデータを出さない）。
- `harness reindex --full` で明示的にフル再索引。

## 5. ストレージ形式（方針: 単一 SQLite ファイル ── 実装で確定。スキーマ詳細は仮）

`$HARNESS_HOME/state/ckg.sqlite`（WAL モード）。

テーブル（以下は仮のスキーマ、実装で確定）:

- `nodes(id, kind, name, qname, file, start_line, end_line, lang, signature, doc, tags)`
- `edges(from_id, to_id, kind, file, line)`
- `meta(key, value)` ── 含 `index_rev`

インデックス（仮）: `nodes.file`, `nodes.qname`, `edges.from_id`, `edges.to_id`, `edges.kind`。

SQLite の理由:

- queryable
- atomic swap 可（temp に build → rename）
- 並行読み OK（WAL）
- サーバ不要
- 1ファイルで配布

`reindex` は temp DB（`ckg.sqlite.new`）に build → `rename()` で atomic swap（同一 FS なら atomic）。並行リーダーは古い DB を持ったままで OK（再接続で新に切替）。

## 6. クエリ API（`harness` コマンド）

これらは harness の一級コマンドではなく、外部/プラガブルな CKG バックエンドへの素通し ── harness 表面では `harness query <name> ...` 経由で呼ぶ（DESIGN 側の再フレームと整合）。例外として `reindex` / `ckg-stale` は CKG キャッシュ artifact の管理なので harness 側のコマンド。以下のコマンド名と意味は正典（DESIGN / schemas からも参照される）。

| コマンド | 引数 | 返すもの | read-only |
| --- | --- | --- | --- |
| `find-symbol` | `<name> [--kind fn\|type\|trait\|const\|...]` | 一致ノード（qname, file:line, kind, signature）。曖昧 or 完全一致 | yes |
| `show-symbol` | `<qname>` | そのシンボルの range のソーステキスト（ファイルを読み行をスライス）＋ signature ＋ doc | yes |
| `outline` | `<file> [--depth N]` | ファイルのシンボルツリー（signature ＋ first-line doc、本体なし）。depth = ネスト深さ | yes |
| `refs` | `<qname>` | 逆 references エッジ（file:line の使用箇所 ＋ 各使用箇所を囲むシンボル） | yes |
| `callers` | `<qname>` | 逆 calls エッジ（file:line の使用箇所 ＋ 各使用箇所を囲むシンボル） | yes |
| `implementers` | `<trait-qname>` | 逆 implements エッジ | yes |
| `deps` | `<module>` | imports 前方 | yes |
| `rdeps` | `<module>` | imports 逆方向 | yes |
| `closure` | `<qname> --depth N` | calls / references の BFS（qname 集合 ＋ それらのファイル ＝ blast radius 候補集合） | yes |
| `impacted-by` | `<qname>` | 逆参照閉包（変えたら壊れうる箇所） | yes |
| `tested-by` | `<qname>` | tested_by エッジ（速いリグレッション gate の対象テスト） | yes |
| `reindex` | `[--full]` | 索引器を叩いて再生成（既定は増分、`--full` でフル）、atomic swap、`index_rev` 更新 | no（CKG artifact を更新） |
| `ckg-stale` | — | `index_rev` vs git HEAD と stale ファイル一覧 | yes |

すべて read-only（run state を触らない）。`reindex` のみ CKG artifact（一種のキャッシュ）を更新。

## 6.1 実装の段階付け（方針 ── 実装で確定）

CKG バックエンドはプラガブル（§冒頭注記）── harness はクエリ IF（`find-symbol` / `refs` / `closure` / …、§6）を定義しデフォルトバックエンドを 1 個出すだけ。Serena / LSP 版 → SCIP+SQLite 版と差し替えていける。段階:

1. **Phase 1.5 でまず「Serena / LSP ブリッジ」で CKG バックエンドを立てる** ── precompute 無し・ライブクエリ・すぐ動く（この環境は Serena 既導入、§3「LSP on-demand 経路」）。`closure --depth N` は安くできない（`references` を N 回）が precompute ゼロ・任意の LSP 対応言語で動く。`outline` はこのときも tree-sitter で出してよい。
2. **`closure` / `impacted-by` の性能が要るようになったら「SCIP 取り込み ＋ SQLite ＋ ファイル単位＋逆依存閉包の増分」（§3 一次・§4・§5）を足す。**

CKG バックエンドの初期実装（Serena / LSP ブリッジ）と後継（SCIP+SQLite）の境界 ── どこまでを Serena 版で済ませ、どの SCIP 索引器をサポートするか ── は実装段階で詰める（`docs/implementation.md` 未決リスト）。

## 6.2 Rust クレートまとめ（方針 ── どれを使うかはバックエンドの実装段階で確定）

- `scip` ── SCIP `.scip`（protobuf）の読み書き（or `.proto` から型生成）。SCIP 取り込み経路で使う。
- `tree-sitter` ＋ `tree-sitter-<lang>`（言語ごとの文法）── 構文構造・`outline`。
- `tree-sitter-stack-graphs`（＋ `stack-graphs`）── tree-sitter パース＋言語ごとの stack graph ルールから名前解決グラフ（参照解決のフォールバック）。
- `lsp-types` ／ `lsp-server` ／ `async-lsp` ── LSP を駆動するなら（Serena / LSP ブリッジ経路）。`lsp-types`＝プロトコル型、`lsp-server`＝JSON-RPC トランスポート、`async-lsp`＝高レベルクライアント。
- `rusqlite`（bundled feature）── CKG の SQLite ストア（SCIP+SQLite 経路）。
- `git2`（or `git` シェルアウト）── 増分再索引で変更ファイルを取得（`git diff --name-only`）。

SCIP 索引器そのもの（`rust-analyzer scip` / `scip-typescript` / …）は外部プロセスにシェルアウトするので Rust クレートではない。

## 7. blast radius 特定への使い方

spec ノードで `harness closure <entrypoint> --depth 2` → 候補集合 → 人間がレビュー（`harness ask` で「この範囲で正しいか? 漏れ / 余分は?」）→ `spec.toml` の `requirement.files` に確定。

`harness impacted-by` で「変えたら壊れうる箇所」、`harness tested-by` で「速いリグレッション gate の対象テスト」を得る。

## 8. traceability gate への使い方

CKG が「どのファイルがどのシンボルを実装し、どのテストがどのシンボルを参照するか」を知ってるので、`traceability_closed` gate がそれを使って下記を機械的に検証できる:

- 各 F-NNN に実在 artifact ≥1 かつ exit 0 する test ≥1
- 登録 artifact がどれかの F-NNN の files に含まれる（orphan なし）

## 9. 正直な限界

- SCIP 索引器は言語ごとに品質 / カバレッジがまちまち。
- SCIP 索引器は対象プロジェクトが*ビルド / 型チェックできる*必要がある（ツールチェーン・依存・設定が揃っていないと索引できない、または部分的にしか索引できない）── CI に索引ステップを置く運用が前提。tree-sitter フォールバック（`outline`・stack-graphs）はビルド不要だが精度が落ちる。
- マクロ・コード生成・動的ディスパッチ・リフレクション・ビルド時設定は過小表現（静的グラフは拾うが動的エッジは漏れる）。
- 多言語 repo は複数索引器のマージが要る（moniker の名前空間で衝突回避、ただし言語境界をまたぐ呼び出し ── FFI 等 ── は表現しにくい）。
- `tested_by` ヒューリスティックは偽陽性（多くのテストから参照される test helper）偽陰性（推移的にしか使わないテスト）あり。
- 増分の逆依存閉包が hot symbol で大きくなる → フルフォールバック。
- → だから「フルスイート遅い gate」（DESIGN §8）が安全網のまま。
