# Deep grill-with-docs — 設計を正してテストを正す

status: design v1（Q1-Q4 確定・フレームワーク接地済み）
関連: ADR-059(独立評価者), SDD(設計先行), design-pre, 差分mutation(下流検証)

## 1. 動機

```
正しいテスト  ←  正しい設計  ←  十分な詰問 (grill-with-docs)
```
AI のテストは「できたつもり」（網羅が生成者の自己申告）に陥る。mutation で**下流の検証層**は
作ったが、根治は上流＝**設計の正しさ**。そこを詰問(grilling)で担保する。

## 2. 核心（当初案の訂正）

当初「詰問のゴール＝testable spec」と置いたが **誤り**だった：
- **testable spec ≠ 正しい設計**。精密な spec でも「間違った物」を設計しうる。
- 詰問のゴールは **正しい設計**。到達点＝**敵対的詰問が loop-until-dry で枯渇**（設計を覆す
  新しい論点が出なくなる）こと。
- testable（AC/INV/具体例）は **下流の sanity check** に格下げ。テストはそこから従属的に決まる。

**限界（overclaim しない）**：
- `正しいテスト ← 正しい設計 ← 深い詰問` は **必要条件であって十分条件ではない**
  （深く問うても人間が誤判断しうる／設計が正しくても網羅の穴は残る）。→ 下流(mutation/カオス)の
  保険は**必須**で、詰問はその残差を**減らす**もの。
- **「枯渇」は“体系的生成を出し切った”であって“設計が健全”の証明ではない**（弱い詰問者は早く
  枯れる＝偽枯渇）。だから生成は固定リスト暗記でなく**生成法の総当たり**で底上げする（mutation の
  equivalent/missed が機械では区別できないのと同じ epistemics）。

## 3. 確定事項（Q1-Q4）

### Q1 停止条件
フィールド充足ではなく、**敵対的詰問の loop-until-dry 枯渇 ＋ その記録**。
「設計を覆す新論点が出なくなるまで止めない」が深さの定義。

### Q2 質問源と網羅
- **doc 由来 主**：`変更(blast radius) × {過去バグ / ADR / architecture / CONTEXT用語 / 品質目標}`
  の交差を質問化（docs を“背景資料”でなく“質問生成器”に＝grill-with-docs の核）。
- **相対網羅は生成法で担保＋単一フレームワークに collapse しない**（「正しい設計」は1軸でない）。
  `要素 × 演算子`の総当たりを **3つの直交レンズ**で：
  1. **損失・失敗軸＝STPA(＋HAZOP)**：`Losses → Hazards → Unsafe Control Actions → Loss Scenarios`
     （安全＋セキュリティ統合＝旧 STPA-Sec。偶発＋敵対の両因。STRIDE/FMEA 不採用）。データ/分散は
     **ACID＋CAP/PACELC＋一貫性モデル**＋fallacies。
  2. **機能・論理の正しさ軸**：損失でなく「ただ間違っている」誤り＝境界/異常/不変条件/property、
     ISO/IEC 25010:2023 機能適合性。← AC/INV の源。
  3. **設計品質軸**：損失でも論理でもない「良い設計か」＝**Ousterhout(深い/浅いモジュール・結合・
     複雑性の隠蔽)**・ドメインモデル整合・抽象/API 一貫性。(harness は deletion_test で既に使う軸を前倒し)
  ※ STPA 一本にしないのは、STPA が「損失を招くか」には強いが「論理が正しいか/良い設計か」を
     生成しないため。単一フレームワークは不完全という本設計の原則(下記4)と整合させる。
- doc が薄い / ADR 0件 → **次元タクソノミ主に反転**し、**詰問が最初の ADR を生む**（前進蓄積）。
- 絶対網羅は原理的に不可能（開放性・決定不能・ISO25010 改訂が反証）→ 残差は **下流保険**
  （mutation / カオス / 人間）が直交層で詰める。

### Q3 詰問深度の可変
- 軸は **blast radius(コード量) ではなく設計リスク**（小×重大＝1行で致命を取りこぼすため）。
- **損失への近さ(STPA)** で測る：この変更は **Loss(許容できない損失)に至る制御動作**に触れるか。
  これが重大性・不可逆性を自然に内包（損失の深刻度＝それ）。
- **OR トリガー**：`損失に近い / 新規性(新Why) / 低検出(下流で気づけない)` のいずれか → **deep**。
  明確に trivial な時だけ軽く。（FMEA/RPN は不採用。検出性は一般原則として残すが FMEA 由来ではない）
- **非対称コスト**（過少詰問 ≫ 過剰詰問）→「迷えば deep」。誤分類は **escalation(途中昇格)＋下流保険**。

### Q4 試作場所
**Crypto .harness で先に試作 → 実機能で実証 → 実証版を scaffold へコピー**。
理由：deep-grilling は**プロンプト型**（質問の質・停止・triage の当たりは実物で反復しないと出ない）。
Crypto は **速いループ（skill=markdown は rebuild 不要）＋ 実材料（実 ADR/バグ/設計判断）**。
skill は最初から tool-agnostic に書き、展開は書き直しでなくコピー。

## 4. フレームワーク接地（なぜこれで良いかの根拠）

1. **完全な軸/次元集合は存在しない**：高利害分野で割れる（HAZOP vs STPA vs ISO25010 改訂…）。
   → 相対網羅＋直交保険が唯一誠実な形。
2. **高利害分野の独立収斂**：プロセス安全(HAZOP)・システム安全(STPA)・テスト(mutation)が
   揃って `要素×演算子` に到達 ＝ 達成可能な天井の証拠。
3. **直交多層の数理**：独立な保険層を重ねると残存見逃しは積で下がる(defense-in-depth)。層は性質を分ける
   （詰問=設計時 / mutation=コード / カオス=運用）。
4. **採用フレームワーク（現行版・3つの直交レンズ）**：単一フレームワークに collapse しない
   （上記1の「完全な集合は無い」と整合。STPA だけでは“論理の正しさ/良い設計”を生成しないため）。
   - **レンズ1 損失・失敗＝STPA**（Leveson, Handbook 2018）：`Losses→Hazards→Unsafe Control Actions
     →Loss Scenarios`。安全＋セキュリティを同じ STPA で（security loss＋偶発/敵対の両因＝旧 STPA-Sec、
     別エンジンでない）。下位道具 **HAZOP ガイドワード(IEC 61882:2016)**。データ/分散＝
     **ACID＋CAP/PACELC＋一貫性モデル**＋fallacies。
   - **レンズ2 機能・論理の正しさ**：境界/異常/不変条件/property、**ISO/IEC 25010:2023**(9特性) 機能適合性。
   - **レンズ3 設計品質**：**Ousterhout**(深い/浅いモジュール・結合・複雑性隠蔽)・ドメインモデル・抽象/API。
   - **不採用**：**STRIDE**（要素別脅威＝損失軸でのパラダイム混在）・**FMEA/RPN**（部品故障・単一故障前提で
     ソフト設計に弱く、本家 AIAG-VDA 2019 も RPN を AP 表に置換）。

## 5. ハーネスへの写像

`research`（既存 grilling＋CONTEXT＋master_design_reviewed）を deep-grilling に強化：
- **interrogator（独立・敵対的）**：質問源(Q2)から「設計を壊す質問」を生成 → AskUserQuestion で人間に
  詰問 → 回答を spec / ADR に反映。実装本人とは別視点（ADR-059 を設計フェーズに前倒し）。
- **深度 triage（Q3 の OR トリガー）**で loop 回数・網羅範囲を可変。
- **停止＝loop-until-dry 枯渇＋interrogation log を evidence 記録**。
- testable(AC/INV/具体例) は plan/design-pre 以降の下流 sanity に置く。

## 6. 段階導入

- **Phase 1（Crypto .harness・skill のみ）✅ 実装済**：`01-research.md` に deep-grilling プロトコル
  （3レンズ・OR トリガー triage・loop-until-dry・記録）を埋め込み。
- **Phase 2（独立詰問者のサブエージェント化）✅ 実装＋ブラインド検証済**：deep 時、B/C の質問生成を
  実装本スレッドでなく独立サブエージェント(`Agent`)に委ね、**「想定する実装/答え」を渡さない**ことで
  後知恵バイアスを構造的に排除。本スレッドは返った問いを人間に詰問し loop-until-dry。
  **検証(2026-06-27)**：edge-d「鳴ったのに入らない」の intent をバグを伏せて独立詰問者に当てたところ、
  リポジトリ/ADR を読まず **HAZOP:No × 価格quote で『片脚WS凍結→偽スプレッド・鮮度ゲート不在』
  ＝実際に出荷された設計穴を最上位 high で自力検出**。契約単位不一致・板深さ・往復コスト・SSOT 二重実装
  （＝実際の根治）等の既知バグ群も同時に列挙。後知恵なしで効くことを確認。
- **Phase 3（実装済・再設計）**：当初の「専用カタログ」は却下（陳腐化＋Phase 2 が generic 失敗クラスは
  カタログ無しで出すと実証）。代わりに**既存記録を on-demand 採掘**：独立詰問者に
  `git log --grep='fix|再発|根治|revert' -- <blast radius>` ＋ 該当 ADR を渡し、「この変更は失敗クラス○○
  （**この repo 固有の傷跡**＝generic には導けない tail）を再来させるか」を生成。記録そのものがカタログ。
  Phase 2(generic) と直交。**検証(2026-06-27)**：edge/ws 領域に採掘を当て、`fix(edge-d):価格鮮度ゲート…WS凍結`
  や `spot canonical 化を全 INSERT 経路に…漏れ修正` 等の project 固有インシデントが surface することを確認。
- **scaffold 展開済**：`skill_templates/01-research.md` に Phase 1+2+3 を tool-agnostic で展開（`harness init` で生成確認）。

### gate 化の決定（deep-grilling を自分の gate 決定にドッグフードして判定・2026-06-27）
「research に `evidence_recorded interrogation`(非ブロッキング)を足して強制する」案を**独立詰問者に当てて却下**：
- existence-only は **自分が保証すべき性質(独立詰問が本当に行われたか)を検証できない**・**triage が light/skip
  でも N/A 逃げ場なく trivial をブロック→捏造誘発**・**深さ決定(triage=skill)と enforcement(gate)の責務分裂**・
  **Goodhart(記録の産出が目的化)**・**後で content 強化しても stub が通る悪い baseline を焼付け**＝**支配された設計**。
- **決定**：existence-only gate は入れない。**skill 推奨を維持**。どうしても gate 化するなら
  **content-gate**（`json_in verdict ∈ {deep_exhausted, light, not_applicable}` ＋ deep は `json_has evaluator==independent`）
  ＋ **triage-aware な not_applicable 逃げ場**にする（その property を実際に assert し trivial を塞がない）。
- 教訓：当初の「非ブロッキング gate」案は誤りで、**deep-grilling 自身が自分の gate 決定の穴を捕捉して救った**
  （＝この仕組みがメタ決定にも効く実証）。

## 7. 効果測定（仕組みの自己評価）
「導入後、**下流 mutation の missed / review 指摘が減るか**」を実機能数件で観測（＝上流の詰問が効いた証拠）。

## 8. リスク / 限界
- 詰問過剰で停滞 → OR トリガーで trivial を軽く（Q3）。
- 軸/次元は原理的に不完全 → 直交保険で残差（Q2/4）。
- interrogator が甘い質問に堕ちる → 独立化＋doc 由来で具体化。
- プロンプト型ゆえ初期は質が低い → Crypto で反復、回すほど鋭化＋ADR 蓄積。
