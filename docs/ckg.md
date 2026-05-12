# CKG — コードナレッジグラフ詳細設計

> `DESIGN.md` §9（context 圧縮戦略）の詳細。索引器・グラフは外部/プラガブル（harness 本体に索引器は内蔵しない）。実装は Phase 1.5（CLI コアの後、ランタイムと並行可）。

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

## 3. 構築

### 一次: SCIP

Sourcegraph の索引フォーマット（LSIF の後継でよりシンプル / 効率的）。各言語の SCIP 索引器にシェルアウト（`rust-analyzer scip` / `scip-typescript` / `scip-python` / `scip-java` 等）→ `.scip` protobuf を CKG ストアに取り込む。

SCIP が定義・参照・実装・docstring・symbol moniker を供給 ── ノード/エッジデータの大半。

### フォールバック（SCIP 索引器が無い言語）

tree-sitter で構文構造（file→symbol 包含、シグネチャ）＋ LSP（`textDocument/definition`, `references`）で意味エッジを遅延計算。

tree-sitter 単体でも outline（シグネチャツリー）は出せる ── 「outline」コマンドはこれで足りる。cross-reference は LSP か SCIP が要る。

### test 検出（tested_by エッジ）

ヒューリスティック ── プロジェクトのテスト glob（`**/tests/**`, `*_test.rs`, `test_*.py` 等、config が宣言）にマッチするファイル内のシンボルが非テストシンボルを参照 → tested_by エッジ。

より精密にしたければ、テストランナーが coverage→symbol マップを出せるならそれを優先。config がテスト glob と任意で coverage-map コマンドを宣言。

### 多言語 repo

複数 `.scip` を1つのストアにマージ ── moniker が言語 / パッケージで名前空間化されてるので衝突しない。

## 4. インクリメンタル更新の粒度（確定: ファイル単位 ＋ 逆依存閉包 ＋ フルフォールバック）

- 再索引の単位は**ファイル**（索引器はファイル or コンパイル単位で動く、単一シンボルでは動かない ── シンボル単位増分はやらない、複雑さに見合わない）。
- `harness reindex`: git に変更ファイルを聞く（`git diff --name-only <last-indexed-rev> HEAD`）→ 変更ファイル**＋その逆依存閉包**（変更ファイル内のシンボルを参照する他ファイル ── CKG の impacted-by で求まる。理由: file A が file B のシンボル S を参照していて B が変わって S が動いたら、A は変わってなくても A→S のエッジを更新する必要がある）を再索引 → CKG ストアにパッチ（変更 / 逆依存ファイルのノード・エッジを削除して再投入）。巨大 repo でもフル再索引よりずっと安い。
- 逆依存閉包自体が大きい場合（広く使われるユーティリティ ＝ "hot symbol"）はフル再索引にフォールバック。索引器が増分非対応ならフル。
- stale 判定: `meta.index_rev` ≠ git HEAD なら一部ファイルが stale → そのファイルへのクエリは `stale: re-index 推奨` フラグ付きで返す（黙って古いデータを出さない）。
- `harness reindex --full` で明示的にフル再索引。

## 5. ストレージ形式（確定: 単一 SQLite ファイル）

`$HARNESS_HOME/state/ckg.sqlite`（WAL モード）。

テーブル:

- `nodes(id, kind, name, qname, file, start_line, end_line, lang, signature, doc, tags)`
- `edges(from_id, to_id, kind, file, line)`
- `meta(key, value)` ── 含 `index_rev`

インデックス: `nodes.file`, `nodes.qname`, `edges.from_id`, `edges.to_id`, `edges.kind`。

SQLite の理由:

- queryable
- atomic swap 可（temp に build → rename）
- 並行読み OK（WAL）
- サーバ不要
- 1ファイルで配布

`reindex` は temp DB（`ckg.sqlite.new`）に build → `rename()` で atomic swap（同一 FS なら atomic）。並行リーダーは古い DB を持ったままで OK（再接続で新に切替）。

## 6. クエリ API（`harness` コマンド）

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

## 7. blast radius 特定への使い方

spec ノードで `harness closure <entrypoint> --depth 2` → 候補集合 → 人間がレビュー（`harness ask` で「この範囲で正しいか? 漏れ / 余分は?」）→ `spec.toml` の `requirement.files` に確定。

`harness impacted-by` で「変えたら壊れうる箇所」、`harness tested-by` で「速いリグレッション gate の対象テスト」を得る。

## 8. traceability gate への使い方

CKG が「どのファイルがどのシンボルを実装し、どのテストがどのシンボルを参照するか」を知ってるので、`traceability_closed` gate がそれを使って下記を機械的に検証できる:

- 各 F-NNN に実在 artifact ≥1 かつ exit 0 する test ≥1
- 登録 artifact がどれかの F-NNN の files に含まれる（orphan なし）

## 9. 正直な限界

- SCIP 索引器は言語ごとに品質 / カバレッジがまちまち。
- マクロ・コード生成・動的ディスパッチ・リフレクション・ビルド時設定は過小表現（静的グラフは拾うが動的エッジは漏れる）。
- 多言語 repo は複数索引器のマージが要る（moniker の名前空間で衝突回避、ただし言語境界をまたぐ呼び出し ── FFI 等 ── は表現しにくい）。
- `tested_by` ヒューリスティックは偽陽性（多くのテストから参照される test helper）偽陰性（推移的にしか使わないテスト）あり。
- 増分の逆依存閉包が hot symbol で大きくなる → フルフォールバック。
- → だから「フルスイート遅い gate」（DESIGN §8）が安全網のまま。
