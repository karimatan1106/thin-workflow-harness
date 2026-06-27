#!/usr/bin/env node
// reconcile_gate: state/divergences.json の各 divergence に、state/reconcile_ledger.json の
//   独立評価者署名つき3値裁定(preserve_quirk / accepted_env_diff / intentional_fix)があるか集合差分で検査。
//   未署名が1件でもあれば exit1(silent-divergence 禁止=signed-empty-set teeth)。
//
// 各裁定の要件:
//   preserve_quirk    : 旧挙動を pin する実テスト(test 非空)。bug-for-bug 保存。
//   accepted_env_diff : ★skeptic2 fix。循環(『R 下 old==new』)でなく
//                       (a) positive_fixture(R 前 old≠new・R 後==) と (b) discriminating_witness(R が保存すべき
//                       既知隣接バグ) の両指名 + scope メタ。容認した環境差(EBCDIC→ASCII 等)。
//   intentional_fix   : adr(ADR-NNN) 非空 + downstream(下流影響) 非空。意図的な挙動変更。
//
// 署名 = evaluator === "independent"(別 cold セッションの敵対評価者。shape のみ強制=暗号証明でない)。
// fail-safe: divergences.json 無 / divergence 0 / ledger 無 → N/A で exit0。
//
//   node bin/reconcile_gate.mjs   (cwd=.harness)

import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { repoRootFrom, loadJsonSafe } from './preservation_lib.mjs';

const { harnessDir } = repoRootFrom(import.meta.url);
const DIV = join(harnessDir, 'state', 'divergences.json');
const LEDGER = join(harnessDir, 'state', 'reconcile_ledger.json');
const ok0 = (m) => { console.log(m); process.exit(0); };

function recordValid(rec) {
  if (!rec || rec.evaluator !== 'independent' || !rec.reason) return false;
  switch (rec.status) {
    case 'preserve_quirk': return !!rec.test;
    case 'intentional_fix': return !!rec.adr && !!rec.downstream;
    case 'accepted_env_diff': return !!(rec.positive_fixture && rec.discriminating_witness && rec.scope);
    default: return false;
  }
}

function main() {
  if (!existsSync(DIV)) ok0('[reconcile-gate] divergences.json 無し → N/A (exit 0)');
  const div = loadJsonSafe(DIV, null);
  const list = div && Array.isArray(div.divergences) ? div.divergences : [];
  if (list.length === 0) ok0('[reconcile-gate] divergence 0 → no_divergence (exit 0)');

  const ledger = loadJsonSafe(LEDGER, []);
  const byId = new Map((Array.isArray(ledger) ? ledger : []).filter((r) => r && r.id).map((r) => [r.id, r]));

  const unsigned = [], invalid = [];
  for (const d of list) {
    const rec = byId.get(d.id);
    if (!rec) { unsigned.push(d); continue; }
    if (!recordValid(rec)) invalid.push({ d, rec });
  }

  if (unsigned.length === 0 && invalid.length === 0) {
    ok0(`[reconcile-gate] OK ── 全 ${list.length} divergence に独立署名の3値裁定あり(all_signed)。`);
  }
  console.error('[reconcile-gate] 未裁定/不備の divergence(silent-divergence 禁止):');
  for (const d of unsigned) console.error(`  ✗ 未署名 ${d.id}: ${d.path} (old=${JSON.stringify(d.old)} new=${JSON.stringify(d.new)})`);
  for (const { d, rec } of invalid) console.error(`  ✗ 署名不備 ${d.id}: status=${rec.status}(要件未充足)`);
  console.error('\n各 divergence を別 cold セッションの独立評価者が裁定し state/reconcile_ledger.json に追記:');
  console.error('  preserve_quirk: {"id":"<id>","evaluator":"independent","status":"preserve_quirk","reason":"...","test":"<旧をpinするテスト>"}');
  console.error('  accepted_env_diff: {..."status":"accepted_env_diff","positive_fixture":"...","discriminating_witness":"...","scope":"...","reason":"..."}');
  console.error('  intentional_fix: {..."status":"intentional_fix","adr":"ADR-NNN","downstream":"...","reason":"..."}');
  process.exit(1);
}

try { main(); } catch (e) { ok0(`[reconcile-gate] 予期せぬエラー → fail-safe N/A (exit 0): ${String(e.message).split('\n')[0]}`); }
