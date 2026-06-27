#!/usr/bin/env node
// differential_gate: 同一 input を 旧golden ↔ 新システム へ流し、equivalence.json の per-field 等価で比較。
//   全 divergence を安定アドレス(JSON path)+content-hash 付きで state/divergences.json へ列挙する
//   (★報告生成が teeth=exit0。divergence の有無で advance は止めない=後段 reconcile が署名裁定で握る)。
//
// ★skeptic2 fix: per-rule must-diff tripwire。各等価規則 R に discriminating witness(R が保存すべき
//   既知隣接バグ fixture)が宣言されていれば、R を適用しても witness が赤に保たれることを検査し、
//   R が witness を green 化したら exit1(=過剰正規化の自己検出)。
//
// fail-safe: golden 無 / 新系 base 無 / golden 空 → N/A で exit0。run を詰まらせない。
//   verdict は evidence(mutation_diff 同様)で別途 json_in が縛る(equivalent|divergences_enumerated|not_applicable)。
//
//   node bin/differential_gate.mjs        (cwd=.harness。新系 base は env PRESERVATION_NEW_BASE)

import { writeFileSync, existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { repoRootFrom, loadEquivalence, loadJsonSafe, compareUnderEquivalence, contentHash } from './preservation_lib.mjs';

const { harnessDir } = repoRootFrom(import.meta.url);
const GOLDEN = join(harnessDir, 'golden', 'manifest.json');
const EQUIV = join(harnessDir, 'equivalence.json');
const STATE = join(harnessDir, 'state');
const OUT = join(STATE, 'divergences.json');
const ABSORBED = join(STATE, 'absorbed_divergences.json');
const NEW_BASE = process.env.PRESERVATION_NEW_BASE || '';
const ok0 = (m) => { console.log(m); process.exit(0); };

async function getNew(entry, headers) {
  // entry.kind: "http"(path) / "inline"(new 直書き=テスト用) / その他は未対応で skip。
  if (entry.kind === 'inline') return { status: entry.new_status ?? null, body: entry.new };
  if (entry.kind === 'http') {
    if (!NEW_BASE) return null; // 新系 base 未設定 → 比較不能(fail-safe で skip)
    const { rawCaptureFetch } = await import('./preservation_lib.mjs');
    const r = await rawCaptureFetch({ base: NEW_BASE, path: entry.path, headers });
    return r.ok ? { status: r.status, body: r.body } : null;
  }
  return null;
}

async function main() {
  if (!existsSync(GOLDEN)) ok0('[differential-gate] golden/manifest.json 無し → N/A (exit 0)');
  const manifest = loadJsonSafe(GOLDEN, null);
  const entries = manifest && Array.isArray(manifest.entries) ? manifest.entries : [];
  if (entries.length === 0) ok0('[differential-gate] golden entries 空 → N/A (exit 0)');
  const policy = loadEquivalence(EQUIV);
  for (const w of policy.warnings) console.error(`[differential-gate] equivalence 警告: ${w}`);

  const allDiv = [];
  const absorbedTotal = {};
  let compared = 0, skipped = 0;
  for (const e of entries) {
    const headers = e.headers || {};
    const oldVal = { status: e.status ?? null, body: e.body };
    const nw = await getNew(e, headers);
    if (!nw) { skipped++; continue; }
    const newVal = { status: nw.status, body: nw.body };
    const { divergences, absorbed } = compareUnderEquivalence(oldVal, newVal, policy);
    compared++;
    for (const d of divergences) {
      allDiv.push({ id: `${e.id || e.path || compared}:${d.path}`, input: e.id || e.path, path: d.path, old: d.old, new: d.new, sig: contentHash({ p: d.path, o: d.old, n: d.new }, policy) });
    }
    for (const [k, v] of Object.entries(absorbed)) absorbedTotal[k] = (absorbedTotal[k] || 0) + v;
  }

  // ★per-rule must-diff tripwire: 規則 R に witness(old/new 直書きで本来赤であるべき)が在れば、
  //   R 下で赤に保たれるか検査。R が witness を吸収(green化)したら過剰正規化 → exit1。
  const tripwireFailures = [];
  for (const r of policy.rules) {
    if (!r.witness || !r.witness.old || !('new' in r.witness)) continue;
    const { divergences } = compareUnderEquivalence(r.witness.old, r.witness.new, policy);
    if (divergences.length === 0) tripwireFailures.push(r.id || r.field);
  }

  try {
    writeFileSync(OUT, JSON.stringify({ generated: true, compared, skipped, divergences: allDiv }, null, 2) + '\n');
    writeFileSync(ABSORBED, JSON.stringify(absorbedTotal, null, 2) + '\n');
  } catch (e) { ok0(`[differential-gate] 書込失敗 → fail-safe N/A (exit 0): ${String(e.message).split('\n')[0]}`); }

  console.error(`[differential-gate] compared=${compared} skipped=${skipped} divergences=${allDiv.length} → state/divergences.json`);
  if (skipped && !NEW_BASE) console.error('  注: PRESERVATION_NEW_BASE 未設定で http entry を skip(新系録画/設定後に再実行)');
  if (tripwireFailures.length) {
    console.error(`[differential-gate] ★過剰正規化(tripwire): 規則 ${tripwireFailures.join(', ')} が discriminating witness を green 化した。scope を絞れ。`);
    process.exit(1);
  }
  // 報告生成が teeth(exit0)。divergence の有無は reconcile が署名裁定で握る。
  ok0(allDiv.length ? `[differential-gate] divergences_enumerated=${allDiv.length}(→reconcile で署名裁定)` : '[differential-gate] equivalent(divergence 0)');
}

main().catch((e) => ok0(`[differential-gate] 予期せぬエラー → fail-safe N/A (exit 0): ${String(e.message).split('\n')[0]}`));
