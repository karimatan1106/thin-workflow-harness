# Design-writing — 正しい設計を「手戻りが出ない形」で書く

status: design v1（①②③ 確定・①中核をブラインド2回で検証済み）
関連: [deep-grilling-design.md](./deep-grilling-design.md)（上流＝要件の詰問）, ADR-059(独立評価者), design-pre/implement ノード, 差分mutation(下流検証)

## 1. 動機とチェーン上の位置

```
正しいテスト  ←  正しい設計  ←  網羅的な要件  ←  deep-grill-with-docs
                  ↑ ここ（本書）        ↑ deep-grilling-design.md で担保済み
```

deep-grill で**要件**は網羅的に出せるようになった。次の環＝**その要件から設計をどう「書く」か**。

**真の目的（ここを取り違えない）**：「設計を正しく書く」は手段。本当の目的は
> **LLM 駆動開発で、不確定要素を排除し、安定・低手戻り・最高効率で 設計→開発→テスト を回す。**

だから設計の良し悪しは「正しさ」でなく **「手戻りをどれだけ消すか」** で測る。

## 2. 核心原理：手戻りは2因子の積

```
手戻り量  ∝  (実装LLMが抱える "不確定な自由度")  ×  (誤りが見つかるまでの遅さ)
```

LLM 駆動特有の手戻り源：
- **設計の隙間を LLM が "自信を持って" 誤って埋める**（人間は訊くが LLM は黙って仮定する）
- **長い実装中に LLM が設計意図を drift / misread する**
- **誤りが実装後・出荷後に発覚する**（遅いほど手戻りが巨大）

→ 設計の仕事は3方向：**① 自由度を潰す ／ ② 早く安く捕まえる ／ ③ LLM に prior で上書きさせない**。
3つとも収束先は同じ＝**consequential な決定を機械検査される形に落とす**。①が「何を閉じるか」、②が「いつ捕まえるか」、③が「LLM に勝たせない書き方」。

## 3. ① 自由度の閉鎖（手戻り第1因子）

### done 定義（形で測らない）
> 設計が done ＝ **答えを知らない独立 "実装 LLM" が、その設計だけで、consequential な決定を一つも推量せず実装でき、それについて質問もゼロ**。

「網羅感」や arc42 セクション充足で測らない（deep-grill の「枯渇 not フィールド」と同じ epistemics）。

### 測り方＝独立実装者ドッグフード（deep-grill 独立詰問者の双子）
- 設計を**答えを伏せて**独立サブエージェント(実装者)に渡し、「実装せよ。ただし**設計が決めていない所で推量した点・質問したい点を全部報告せよ**」。
- 返る **assumption（黙って埋めた所）＋question ＝設計が閉じ損ねた自由度**。
- **LLM 固有の手戻り源（自信ある誤推量）を直撃**：黙って推量せず assumption として表に出させると、本番実装 LLM が黙ってやる誤りが設計時に可視化される。

### consequential / mechanical の境界＝手戻りの blast radius
「重要さ」でなく **誤った時の手戻りの広がり**で引く。これは Ousterhout の情報隠蔽境界そのもの：

| | consequential（設計で閉じる） | mechanical（LLM に委ねる） |
|---|---|---|
| 何 | インターフェース境界を越え**他が依存する契約** | インターフェース内側に**隠蔽・局所** |
| 例 | API/シグネチャ・データスキーマ/永続形式・プロトコル・**エラー/失敗の契約**・不変条件・**モジュール分解(責務の所在)**・**AC が観測する挙動** | 関数内アルゴリズム/データ構造・変数名/スタイル・private helper 構成・ログ文 |
| 誤った時 | callers/tests/data/他モジュールに**波及**・移行が要る | **その1箇所の編集で直る** |

**運用テスト（LLM が適用できる形）**：
> 「この決定を後で変えたら、その1箇所を超えて波及するか？（caller を壊す/テストを壊す/スキーマ移行が要る/他モジュールが直る）」
> YES → consequential（閉じる） ／ NO → mechanical（実装裁量と明記して残す）

### 過剰仕様の罠（効率＝目的の片輪）
**自由度を全部潰すと設計＝コードになり効率が死ぬ**。閉じるのは **consequential だけ**、mechanical は LLM に委ねる。
境界限界：(a) 何が「内側」かは**分解の引き方**に相対的＝**最初の consequential 決定は分解そのもの**。(b) 可逆性は連続でグレーゾーン→**非対称デフォルト「迷えば閉じる」**（cheap を閉じる無駄 < costly を開ける手戻り）、ただし過剰仕様コストで上限。(c) **AC 観測点は自動で consequential**（テストが依存＝②と連結）。

### 停止
独立実装者の出す **consequential な assumption/question がゼロ**になるまで設計を閉じる（loop。mechanical は「実装裁量」と明記して残す）。deep-grill の loop-until-dry の設計版。

## 4. ② fail-fast の経済（手戻り第2因子＝遅さ）

手戻りコストは発覚段で指数的に増える：
```
設計時 ≪ コンパイル時 ≪ テスト時 ≪ 出荷後
(段落書き直す)  (型エラー)   (落ちる)   (インシデント+hotfix+やり直し)
```
→ 設計を「より早い段で検査できる形」に書く。同じ誤りなら左で捕まえるほど安い。

| 段 | レバー | 設計に要る性質 |
|---|---|---|
| 設計時(最安) | 独立レビュー＋①の実装者ドッグフード | **falsifiable**＝「適切に処理」でなく**明示の不変条件/契約/「失敗Xを手段Yで防ぐ」**。覆せる主張だけ設計時に検査できる |
| コンパイル時 | 型/スキーマで弾く | **consequential 決定を型・スキーマ・契約に encode**（不正な状態を表現不能に）＝誤実装がコンパイルを通らない |
| テスト時 | 設計からテスト導出 | **各 AC→テスト・各不変条件→assertion・各契約→契約テスト**が機械的に導出可能な粒度 |

**核心レバー＝決定的検査に寄せる**：LLM 駆動で最も信頼できる検査は決定的なもの（型/スキーマ/テスト）で LLM 自己レビューでない（ADR-059 と整合）。consequential 決定は散文でなく機械が enforce できる形で書く。①で閉じた決定を**型に焼く**と閉鎖が機械強制される。

**罠（LLM 固有）**：LLM は設計から各 AC をテストに機械変換できる＝速いが、**それ自体が「できたつもり」の再来**。導出した fail-fast テストは**下流の差分 mutation で誤検出能力を確認**して初めて信頼できる。②（速く導出）と test-quality（mutation で検証）はセット。

## 5. ③ LLM 可読性・context 耐性（LLM 固有の不確定）

### 核心
実装 LLM は**今 context に在る物しか知らず、設計が黙ると prior に流れる**。drift/misread は「忘れる」でなく **設計より prior が勝つ**現象。

| drift 源 | 設計の書き方での対処 |
|---|---|
| context 減衰 | **使用箇所に局所化・implement 時に再提示**（research の記憶に頼らない） |
| 曖昧→prior へ流れる | **prior に逆らって書く**：通念から外れる所を明示（「cache に見えるが eviction を足すな・理由Z」） |
| 構造の誤読 | **原子的・走査可能**：consequential 決定を段落に埋めず離散ラベル付き項目で |
| 矛盾 | **SSOT**：1決定1箇所。乖離する重複を作らない |
| stale | load-bearing 制約は**コード(型/テスト)に置く**・prose に STALE マーカー |

### 支配レバー（②を補強）
> prose 設計は LLM にとって本質的に drift する。型/テスト/スキーマに encode した制約は drift しない。

prose に残すのは **WHY・理由・通念からの逸脱**だけ（型に焼けない物）。それを原子的/局所/anti-prior/SSOT で書く。

### ハーネス側責務
③ は文書だけで閉じない。**implement ノードが consequential な設計制約を再提示する**（research の記憶を当てにしない）。harness の master_design/CONTEXT に consequential 決定を載せる。

### 接地
③ は新発明でなく project 既存ルール（SSOT・How=コード・STALE・200行）を **「LLM drift を防ぐ」動機で operationalize** したもの。

## 6. 検証（①中核をブラインドで撃った）

「うまくいくか」を断言でなく、deep-grill と同じブラインド検証で確かめた。過去に**実際に手戻りした案件**の設計を結果を伏せて独立実装者に渡し、手戻りを生んだ自由度を事前 surface できるか見た。

- **題材＝fr_arbitrage の Discord スクショ機能**（実際に3ラウンド修正した）。
- **汚染実行**：現コードを読ませたら #1（entry=now→空画像）を当てたが、コードに過去修正の**警告コメントが残存**＝「読んだ」可能性で #1 を弱い証拠に格下げ。ただし **#3「画像=レンダー時刻のライブ状態 ≠ アラート時の状態」はコメントに無い subtle 穴を自力導出**＝汚染なしの強い証拠。
- **クリーン実行（frontend のみ・Rust 警告遮断）**：`useTradeAnalysisData computeWindow() startMs=entryMs, endMs=now` を読み、**警告コメント無しで #1「entry=now→窓ゼロ→空画像」を自力導出**＝事前基準クリア。さらに `+00:00`→URL で `+`→space→`new Date` NaN のエンコード罠、4パラメータ必須、settle 信号不在→DOM アンカー待ち、取引所スラグ小文字、symbol canonical、ピクセル固定クロップ破綻 等、**3ラウンド分のバグ史をほぼ全て frontend だけから再構成**。

→ **①（独立実装者ドッグフードで consequential 自由度を事前 surface）は機能する**と確認（deep-grill 詰問者＋汚染 #3＋クリーン #1 の独立3例）。

## 7. 限界（overclaim しない）

- **必要条件であって十分でない**：①②③ は予見可能な LLM 手戻り（隙間の推量・遅い発覚・drift）を**減らす**。設計判断そのものの誤り・予見不能は残る → **下流保険（差分 mutation/テスト/カオス）は必須**。
- **②は一貫性を守るが正しさは守らない**：`FreshQuote` 型は鮮度チェックが全経路に効くことを保証するが、**閾値が正しいか・どの age を測るか**は守れない。その正しさは①/deep-grill 側の責務。
- **境界判定を LLM が誤りうる**（consequential を mechanical と誤分類）→ 非対称デフォルト＋下流保険。

## 8. ハーネスへの写像

- **research（deep-grilling）**：要件＝設計を壊す論点を詰問で枯らす（既存）。
- **design-pre**：本書の①②③で設計を書く。
  - ① deep 変更時、**独立実装者ドッグフード**で consequential 自由度を surface → 閉じる（mechanical は「実装裁量」明記）。
  - ② consequential 決定を**型/スキーマ/契約**に encode、設計主張を falsifiable に、AC→テスト導出可能に。
  - ③ 残留 prose を原子的/局所/anti-prior/SSOT、consequential 決定を master_design に明示。
  - 停止＝独立実装者の consequential assumption がゼロ。
- **implement**：design-pre が出した **consequential 設計制約を再提示**してから実装（③のハーネス責務）。
- **test**：design から導出したテストを差分 mutation で検証（②の罠対策・既存ゲート）。

## 9. 段階導入

- **Phase 1（Crypto .harness・skill のみ）✅ 実装済**：02-plan に①②（独立実装者ドッグフード/境界/
  機械検査焼き/非ブロッキング `design_closure` 記録）、04-implement に③（consequential 設計の再提示）。
  新規 gate は足さない（existence-only gate は支配的設計として却下＝§1）。
- **Phase 2（独立実装者ドッグフードのサブエージェント化）✅ Phase 1 に統合済**：deep-grill と違い
  最初から `Agent` 委譲で書いた（答えを伏せて渡し assumption/question を回収＝後知恵バイアス排除）。
- **scaffold 展開 ✅ 完了**：tool-agnostic で `skill_templates/{02-plan,04-implement}.md` にコピー →
  rebuild + reinstall → `harness init` で生成確認（validate 10 ノード・290 test green）。全 `harness init` に伝播。
- **継続課題（ブロッカーでない）**：§7 の full-loop 効果測定（導入後に下流 mutation の missed /
  review 指摘が減るか）は実機能を回し続けて観測する。①中核はブラインド2回＋実バグ#3 捕捉で測定済み。

## 10. リスク
- ドッグフード過剰で停滞 → deep 変更時のみ（深度 triage は deep-grilling と共有）。
- 型に焼きすぎて設計=コード化 → consequential のみ閉じる（§3 過剰仕様の罠）。
- 独立実装者が甘い → 答えを渡さない・blast radius 境界を明示・doc 由来で具体化。
