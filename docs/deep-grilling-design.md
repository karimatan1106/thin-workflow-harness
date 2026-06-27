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

## 3. 確定事項（Q1-Q4）

### Q1 停止条件
フィールド充足ではなく、**敵対的詰問の loop-until-dry 枯渇 ＋ その記録**。
「設計を覆す新論点が出なくなるまで止めない」が深さの定義。

### Q2 質問源と網羅
- **doc 由来 主**：`変更(blast radius) × {過去バグ / ADR / architecture / CONTEXT用語 / 品質目標}`
  の交差を質問化（docs を“背景資料”でなく“質問生成器”に＝grill-with-docs の核）。
- **相対網羅は生成法で担保**：固定リストは不完全。`要素 × 演算子`の総当たりで相対網羅。
  **生成の背骨＝STPA(1手法)**：`Losses → Hazards → Unsafe Control Actions(出ない/誤って出る/
  タイミング異常/途中で止まる) → Loss Scenarios`。**安全とセキュリティを1手法で扱う**＝Losses に
  security loss も入れ、Loss Scenarios で**偶発故障＋敵対動作の両因**を問う（これが STPA-Sec の眼目。
  別エンジンではないので STPA 一本で良い）。
  下位道具＝**HAZOP ガイドワード(IEC 61882)** で UCA を出す。品質レンズ＝**ISO/IEC 25010:2023(9特性)**、
  データ＝**ACID ＋ CAP/PACELC ＋ 一貫性モデル**、分散＝fallacies。STRIDE は不採用(パラダイム混在回避)。
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
4. **採用フレームワーク（現行版）**：
   - **STPA**（Leveson, Handbook 2018）＝背骨（1手法）。制御構造の不安全動作・相互作用・
     ソフト由来の創発故障を捉える＝「設計が損失を招くか」を問う＝本目的に最適。
     **安全＋セキュリティを同じ STPA で**扱う：Losses に security loss を含め、Loss Scenarios で
     偶発故障＋敵対動作の両因を問う（これが従来 STPA-Sec と呼ばれた適用。別エンジンではないので STPA 一本）。
   - **HAZOP ガイドワード**（IEC 61882:2016）＝UCA 生成の下位道具。
   - **ISO/IEC 25010:2023**（9特性：Interaction Capability / Flexibility / Safety 追加。旧 Usability/
     Portability から改称）＝品質レンズ。
   - **ACID ＋ CAP/PACELC ＋ 一貫性モデル**＝データ/分散の正しさ。分散 fallacies。
   - **STRIDE は不採用**（制御理論の背骨に要素別脅威リストを接ぎ木するパラダイム混在を避ける。
     セキュリティは上記のとおり STPA の損失/敵対シナリオで扱う）。
   - ※ **FMEA は不採用**：部品故障・単一故障前提でソフトの設計正否に弱く、RPN も本家 AIAG-VDA 2019 で
     AP 表に置換された経緯あり。STPA がソフト設計には正道。

## 5. ハーネスへの写像

`research`（既存 grilling＋CONTEXT＋master_design_reviewed）を deep-grilling に強化：
- **interrogator（独立・敵対的）**：質問源(Q2)から「設計を壊す質問」を生成 → AskUserQuestion で人間に
  詰問 → 回答を spec / ADR に反映。実装本人とは別視点（ADR-059 を設計フェーズに前倒し）。
- **深度 triage（Q3 の OR トリガー）**で loop 回数・網羅範囲を可変。
- **停止＝loop-until-dry 枯渇＋interrogation log を evidence 記録**。
- testable(AC/INV/具体例) は plan/design-pre 以降の下流 sanity に置く。

## 6. 段階導入

- **Phase 1（Crypto .harness・skill のみ・rebuild 不要）**：`01-research.md` に deep-grilling プロトコル
  （interrogator役・質問源・OR トリガー triage・loop-until-dry・記録）を埋め込み、**実機能で試す**。
- **Phase 2**：独立 interrogator をサブエージェント化（評価者分離を設計フェーズの前段に）。
- **Phase 3**：docs / 過去バグカタログから質問を自動列挙（grill-with-docs の自動化）。
- 実証後 → scaffold（`skill_templates/01-research.md` 等）へコピー、必要なら gate 追加。

## 7. 効果測定（仕組みの自己評価）
「導入後、**下流 mutation の missed / review 指摘が減るか**」を実機能数件で観測（＝上流の詰問が効いた証拠）。

## 8. リスク / 限界
- 詰問過剰で停滞 → OR トリガーで trivial を軽く（Q3）。
- 軸/次元は原理的に不完全 → 直交保険で残差（Q2/4）。
- interrogator が甘い質問に堕ちる → 独立化＋doc 由来で具体化。
- プロンプト型ゆえ初期は質が低い → Crypto で反復、回すほど鋭化＋ADR 蓄積。
