# 対象コードベースの理想構造（harness が操作しやすいコードベースとは）

> これは*助言的*ドキュメント。thin-workflow-harness はこの構造を*要求しない* ── 任意の（雑に構造化された）レガシーコードベースも扱える（`lines_not_increased` / 遅いフルスイート gate / characterization test 等がそのため）。ここに書くのは「この構造なら blast radius・CKG・traceability・並列化が全部スムーズに効く」という*ゴール*状態。10M 行（≈200 行/ファイルなら ~50,000 ファイル）規模の Web アプリを全ファイル ≤200 行で破綻なく管理する設計でもある。

## 1. 原理

1. **トップは横割り（layer）でなく縦割り（domain/feature）**: `controllers/` `models/` `services/` をトップに置くと各々に 12,500 ファイルで探せない。トップは bounded context（`billing/` `catalog/` `identity/` `search/` …）、各 domain が中でレイヤリング。＝モジュラーモノリス / DDD を 10M 行スケールに。

2. **再帰的（フラクタル）分割**: domain が一定サイズ（>~100k 行 or >~500 ファイル）を超えたら同じパターンで sub-domain に割る（`catalog/` → `catalog/{products,categories,inventory,pricing}/`）。木の深さは対数的（10M 行でも深いところで 3〜5 階層）。

3. **1 ファイル＝1 責務**: 200 行キャップが*これを強制*。Service は 2000 行の `OrderService` でなく `order/app/create_order.rs` / `cancel_order.rs` / `apply_discount.rs` ── 1 操作 1 ファイル。Model は太い `User` でなく `user/domain/user.rs`（構造体、≤200）/ `user_validation.rs` / `user_serialization.rs`。「200 行で機械的に切る」でなく「分解されたコードはこの粒度で自然に 50〜150 行になり、200 は『さらに割れ』を捕まえる天井」。

4. **co-location**: テストはコードの隣（`create_order.rs` ＋ `create_order_test.rs`）、遠い `tests/` ミラー木でない。型・スキーマも使う場所の隣。＝blast radius がローカルに収まる。

5. **モジュール間の境界を明示**: 各 domain は*小さな*公開 API（`mod.rs` / `index.ts` ── 公開表面だけ re-export）を出す。domain 間呼び出しはそこ経由のみ、`domains/billing/internal/...` に他 domain から手を突っ込まない。lint or CKG の `imports` エッジ監査で強制可能。＝blast radius が domain 境界で止まる。

6. **パスが修飾名を符号化**: `domains/billing/invoice/app/issue_invoice.rs` ── ファイルの場所＝完全修飾名。`find-symbol` / blast radius / traceability が全部パスで動く。

7. **横断コードは小さく安定な `kernel/`（or `shared/`）に**: HTTP フレームワーク glue、共通エラー型、DB 接続/トランザクション/migration runner、認証ミドルウェア（認可ロジックは各 domain）、ログ/メトリクス。肥大したら smell。

8. **composition root（全 domain を配線する所）は極薄**: 各 domain が*自己登録*（`domains/billing/wire.rs` を root はただ呼ぶだけ）、domain を足しても root が膨らまない。＝アプリ版の「thin harness, fat skills」。

9. **生成コードを隔離**: `<domain>/_generated/` に置いて 200 行ルール免除（codegen 出力の行数は制御できない）、CKG は `generated` タグ。harness の `max_lines` gate は手書きファイルにだけ効く（`artifact_tags` の免除と整合）。

10. **フロントエンドも同じ domain で割る**: `frontend/components/` `pages/` `hooks/` でなく `domains/billing/ui/...`。feature 変更が両側の `domains/billing/` を触る。or 完全 co-location（`domains/billing/{backend,ui}/`）。

## 2. 提案ディレクトリ構成

```
<repo>/
  kernel/                       # 薄く・安定。横断的関心事のみ
    http/                       # フレームワーク glue・middleware（各 1ファイル）
    errors/                     # 共通エラー型
    db/                         # 接続プール・トランザクション・migration runner
    auth/                       # 認証ミドルウェア（認可は各 domain）
    observability/  config/
  domains/
    <domain>/                   # billing, catalog, identity, search, notifications, ...
      api/                      # 外部表面（HTTP handler / GraphQL resolver / RPC）── 1 handler 1ファイル、薄い
      app/                      # ユースケース ── 1操作1ファイル（create_order.rs, ...）
      domain/                   # ドメインモデル・不変条件・ドメインサービス ── 1概念1ファイル群
      data/                     # リポジトリ・クエリ ── 1集約1リポジトリ、クエリは別ファイル
      ui/                       # フルスタックならこの domain のフロントエンド
      _generated/               # codegen 出力（200行免除、CKG tag=generated）
      wire.{rs,ts}              # この domain を composition root に登録する1ファイル
      mod.rs / index.ts         # 公開 API の re-export（小さい）
      *_test.{rs,ts}            # コードと co-location（or __tests__/）
      <subdomain>/              # 大きくなったら同じパターンで再帰分割
  app/                          # composition root ── 薄い
    main.{rs,ts}                # 各 domain の wire を呼びサーバを起動するだけ
    routes.{rs,ts}              # 各 domain.api をマウント（or 各 domain が自己登録）
  e2e/                          # クロス domain の E2E（単体・結合は domain 内に co-location）
  tools/                        # スクリプト・codegen 設定・CI
  docs/domains.md               # domain の地図（どの domain が何を持つか）── パッケージカードの元
```

## 3. なぜ ≤200 行が 10M 行で成り立つか

- 50k ファイル ÷ 20〜50 domain ＝ domain あたり ~1k〜2.5k ファイル → 再帰分割で末端フォルダは数個〜数十ファイル。深さ ~3〜5。パスが場所を教えるから探せる。
- 200 行は*自然*（1 ファイル＝1 操作/概念/handler/クエリなら 50〜150 行）。200 に近づいたら「コヒーレントな小片を別ファイルに抽出」（ヘルパ・サブケース・ビルダ）── 空行を削るんじゃない。harness の `max_lines` gate がこの規律を強制し、skill が「200 超なら抽出すべき単位を見つけよ」と指示。

## 4. thin harness との相性

- **blast radius が domain 境界で bounded**: billing の変更はほぼ `domains/billing/...` だけ。CKG の `imports` エッジ（domain 間は公開 API 経由のみ）で `impacted-by` が精密。`closure --depth 2` が billing ＋ kernel ＋ 他 domain の公開 API 数個に収まる。
- **CKG フレンドリ**: パス＝修飾名、小ファイル、public/internal が明示（CKG が `public`/`internal` タグ）。`outline <domain>` で domain の表面が数十行で出る。
- **traceability フレンドリ**: 要件 F-NNN が domain（or sub-domain）に対応、`requirement.files = ["domains/billing/..."]`。全ファイルがちょうど 1 domain 配下なので orphan 検出が効く。
- **並列フレンドリ**: disjoint な domain への 2 変更 → disjoint な blast radius → `blast_radius_disjoint` pass → harness が並列化できる（別 worktree or fork/join）。
- **`_generated/` 免除** ＝ harness の per-tag gate 設定（`generated` タグは `max_lines` 不適用）と一致。

## 5. 正直な但し書き

- ~50k ファイルはツール（IDE・git・ビルドシステム）に重い。**ビルド単位とディレクトリ構造を揃える**（domain ≈ crate/package/build unit ── Rust なら workspace で domain ごとに crate、フロントは Nx の package、or Bazel/Buck）── でないと world recompile。
- 200 行は普遍の真理でない。巨大 match/switch、生成パーサ、設定スキーマは 1 つの大きいファイルの方が明快なことがある。ルールは common case の forcing function であって教義でない ── 生成コード ＋ 小さな「意図的に長い」ファイルの allowlist（宣言＋正当化付き）は免除（harness の `lines_not_increased` for legacy と整合）。
- 横断変更（kernel の型のリネーム、全所で使われてる）は*やはり*波及する。構造で防げるのは*domain*変更の波及だけ。CKG の `impacted-by` で波及を見つけ、harness の「フルスイート遅い gate」が安全網。
- 既存 10M 行アプリをこの構造に移行するのは巨大事業 ── 構造は*ゴール*であって一晩で flip するものでない。strangler-fig で domain を切り出していく。harness はこの理想構造でない既存コードベースも扱える（前述）。
