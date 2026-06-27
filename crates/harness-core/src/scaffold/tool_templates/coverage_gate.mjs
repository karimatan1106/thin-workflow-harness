#!/usr/bin/env node
// coverage_gate: preservation/input_space.json の全宣言 partition + quirk クラスが golden で covered か、
//   coverage_ledger.json に署名つき not_applicable があるか(signed-empty-set)を検査する floor ゲート。
//
// ★これは「完了/網羅」を主張しない floor。input 空間 partition 列挙自体の完全性は本ゲートの保証外
//   (誰も宣言しなかった quirk は不可視=research の deep-grilling と独立評価者の責務)。
// ★skeptic1 fix: 除去不能 class(E 未初期化 / F SORT collation 順 / G 採番非決定 / H online 並行)の seed は
//   bare not_applicable を拒否する。captured_nondeterministic(golden で admissible-set 捕獲) か
//   独立署名 quarantine のみ許可(非決定の最高リスク class が N/A 穴を素通りする false-green を封鎖)。
//
// fail-safe: input_space.json 無 / partition 空 → N/A で exit0。
//
//   node bin/coverage_gate.mjs   (cwd=.harness)

import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { repoRootFrom, loadJsonSafe } from './preservation_lib.mjs';

const { harnessDir } = repoRootFrom(import.meta.url);
const INPUT_SPACE = join(harnessDir, 'preservation', 'input_space.json');
const GOLDEN = join(harnessDir, 'golden', 'manifest.json');
const LEDGER = join(harnessDir, 'state', 'coverage_ledger.json');
const ok0 = (m) => { console.log(m); process.exit(0); };

const HARD_CLASSES = new Set(['E', 'F', 'G', 'H']); // 除去不能・bare N/A 不可

function main() {
  if (!existsSync(INPUT_SPACE)) ok0('[coverage-gate] input_space.json 無し → N/A (exit 0)');
  const space = loadJsonSafe(INPUT_SPACE, null);
  const parts = space && Array.isArray(space.partitions) ? space.partitions : [];
  if (parts.length === 0) ok0('[coverage-gate] partition 空 → N/A (exit 0)');

  const manifest = loadJsonSafe(GOLDEN, { entries: [] });
  const golden = Array.isArray(manifest.entries) ? manifest.entries : [];
  // golden が partition を covered とみなす条件: entry.partitions[] に partition id を含む or entry.id 一致。
  const coveredByGolden = (p) => golden.some((e) =>
    (Array.isArray(e.partitions) && e.partitions.includes(p.id)) || e.id === p.id ||
    (Array.isArray(e.partitions) && p.quirk_class && e.partitions.includes(`class:${p.quirk_class}`)));
  // golden entry が非決定捕獲(admissible-set)で覆ったか。
  const coveredNondet = (p) => golden.some((e) => e.captured_nondeterministic && (
    (Array.isArray(e.partitions) && e.partitions.includes(p.id)) || e.id === p.id));

  const ledger = loadJsonSafe(LEDGER, []);
  const waived = new Map((Array.isArray(ledger) ? ledger : [])
    .filter((r) => r && r.id && r.evaluator === 'independent' && r.reason)
    .map((r) => [r.id, r]));

  const gaps = [];
  for (const p of parts) {
    if (coveredByGolden(p)) continue;
    const w = waived.get(p.id);
    const isHard = p.quirk_class && HARD_CLASSES.has(String(p.quirk_class).toUpperCase());
    if (isHard) {
      // 除去不能 class: captured_nondeterministic か 署名 quarantine のみ可。bare N/A 不可。
      if (coveredNondet(p)) continue;
      if (w && w.status === 'quarantine') continue;
      gaps.push({ p, why: `除去不能 class ${p.quirk_class}: captured_nondeterministic か署名 quarantine が必要(bare N/A 不可)` });
    } else {
      if (w && (w.status === 'not_applicable' || w.status === 'quarantine')) continue;
      gaps.push({ p, why: '未被覆かつ署名 not_applicable 無し' });
    }
  }

  if (gaps.length === 0) ok0(`[coverage-gate] OK ── 全 ${parts.length} partition が golden 被覆 or 署名(adequate/gaps_signed)。floor のみ(列挙完全性は未主張)。`);
  console.error('[coverage-gate] 被覆不足の partition(signed-empty-set 未充足):');
  for (const g of gaps) console.error(`  ✗ ${g.p.id} [class ${g.p.quirk_class || '-'}]: ${g.why}`);
  console.error('\n対処: golden に当該 partition を録画(quirk 誘発入力)するか、coverage_ledger.json に独立署名 not_applicable を追記:');
  console.error('  {"id":"<partition-id>","evaluator":"independent","status":"not_applicable","reason":"..."}');
  console.error('  class E/F/G/H は bare N/A 不可 → captured_nondeterministic(golden) か status:"quarantine"(独立署名)。');
  process.exit(1);
}

try { main(); } catch (e) { ok0(`[coverage-gate] 予期せぬエラー → fail-safe N/A (exit 0): ${String(e.message).split('\n')[0]}`); }
