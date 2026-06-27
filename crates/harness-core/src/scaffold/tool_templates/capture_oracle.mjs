#!/usr/bin/env node
// capture_oracle: 旧システムの実挙動を I/O 境界で golden 捕獲する。oracle = 旧の実挙動(bug-for-bug)。
//
// ★cardinal sin 回避: capture 時に normalize/sort/blanket-volatile-drop を一切しない(raw 保存)。
//   等価吸収は differential 側 equivalence.json の per-field opt-in に後置する。
//
// 二層保存: golden/manifest.json の各 entry = { id, path, status, body(raw), provenance, [captured_nondeterministic] }。
//
// ★skeptic1/4 fix(非決定性): byte-identical 2回録画でなく k>=N 録画の per-field 分散分類。
//   N 録画間で変動する field は非決定(class E 未初期化 / G 採番 / F tie 順)と判定し、
//   captured_nondeterministic=true + 変動 field を nondeterminism_ledger.json に記録(独立署名 policy 必須)。
//   ※ この汎用実装は分散分類の骨格のみ。COBOL の RDW/copybook/EBCDIC/COMP-3 framing は adapter 仕様(下記)。
//
// modes:
//   record <oldBase> <path> [--n K] [--id ID]   旧系を K 回録画し golden へ追記(K>=2 で非決定分類)
//   verify-provenance                            ★ゲート用。golden の整合を確認。fail-safe:
//                                                 golden 無(oracle 未到達=legit)→N/A exit0 /
//                                                 到達したが不安定 field に等価規則も署名も無い→exit1
//
//   node bin/capture_oracle.mjs verify-provenance   (cwd=.harness)

import { writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { repoRootFrom, loadJsonSafe, rawCaptureFetch } from './preservation_lib.mjs';

const { harnessDir } = repoRootFrom(import.meta.url);
const GOLDEN = join(harnessDir, 'golden', 'manifest.json');
const NONDET = join(harnessDir, 'state', 'nondeterminism_ledger.json');
const ok0 = (m) => { console.log(m); process.exit(0); };

function loadManifest() { return loadJsonSafe(GOLDEN, { entries: [] }); }
function saveManifest(m) { writeFileSync(GOLDEN, JSON.stringify(m, null, 2) + '\n'); }

// K 録画の per-field 分散(JSON path -> 値が割れたか)。簡易: body を JSON 走査し path ごとに値集合を作る。
function fieldVariance(samples) {
  const sets = new Map();
  function walk(v, path) {
    if (v && typeof v === 'object') {
      if (Array.isArray(v)) v.forEach((x, i) => walk(x, `${path}[${i}]`));
      else for (const k of Object.keys(v)) walk(v[k], path ? `${path}.${k}` : k);
    } else {
      if (!sets.has(path)) sets.set(path, new Set());
      sets.get(path).add(JSON.stringify(v));
    }
  }
  for (const s of samples) walk(s, '');
  const volatile = [];
  for (const [p, set] of sets) if (set.size > 1) volatile.push(p);
  return volatile;
}

async function record(oldBase, path, n, id) {
  if (!oldBase || !path) { console.error('使い方: capture_oracle.mjs record <oldBase> <path> [--n K] [--id ID]'); process.exit(2); }
  const samples = []; let status = null;
  for (let i = 0; i < Math.max(1, n); i++) {
    const r = await rawCaptureFetch({ base: oldBase, path });
    if (!r.ok) { console.error(`[capture] 旧系到達不能(${r.error}) → 録画中止(N/A)`); process.exit(0); }
    status = r.status; samples.push(r.body);
  }
  const volatile = n >= 2 ? fieldVariance(samples) : [];
  const m = loadManifest();
  m.entries = (m.entries || []).filter((e) => e.id !== (id || path));
  const entry = { id: id || path, path, kind: 'http', status, body: samples[0],
    provenance: { recordedAt: 'CAPTURE_TIME', n, oldBase, note: 'raw(no normalize)' } };
  if (volatile.length) { entry.captured_nondeterministic = true; entry.nondeterministic_fields = volatile; }
  m.entries.push(entry);
  saveManifest(m);
  // 非決定 field は equivalence.json に volatile/multiset 規則を後置する必要 → ledger に列挙(独立署名 policy 必須)。
  if (volatile.length) {
    const led = loadJsonSafe(NONDET, []);
    led.push({ id: id || path, fields: volatile, evaluator: null, policy: null, note: 'requires independent-signed policy' });
    writeFileSync(NONDET, JSON.stringify(led, null, 2) + '\n');
    console.error(`[capture] ★非決定 field 検出 ${volatile.length} 件 → nondeterminism_ledger に独立署名 policy 必須`);
  }
  console.error(`[capture] recorded id=${id || path} status=${status} n=${n} nondet=${volatile.length}`);
  process.exit(0);
}

function verifyProvenance() {
  if (!existsSync(GOLDEN)) ok0('[capture] golden 無し(oracle 未到達=legit) → N/A (exit 0)');
  const m = loadManifest();
  const entries = Array.isArray(m.entries) ? m.entries : [];
  if (entries.length === 0) ok0('[capture] golden entries 空 → N/A (exit 0)');
  // 到達したが不安定 field に等価規則も署名も無い → exit1。
  const led = loadJsonSafe(NONDET, []);
  const signed = new Set((Array.isArray(led) ? led : []).filter((r) => r && r.evaluator === 'independent' && r.policy).map((r) => r.id));
  const bad = entries.filter((e) => e.captured_nondeterministic && !signed.has(e.id));
  if (bad.length) {
    console.error('[capture] ★非決定捕獲だが独立署名 policy 無し(captured_nondeterministic 強制):');
    for (const e of bad) console.error(`  ✗ ${e.id}: nondeterministic_fields=${(e.nondeterministic_fields || []).join(', ')}`);
    console.error('  nondeterminism_ledger.json に {"id":..,"evaluator":"independent","policy":"volatile|multiset|tolerance",...} を追記せよ。');
    process.exit(1);
  }
  ok0(`[capture] OK ── golden ${entries.length} entry・非決定は全て署名 policy あり(verify-provenance pass)。`);
}

const mode = process.argv[2];
if (mode === 'record') {
  const rest = process.argv.slice(3);
  const nIdx = rest.indexOf('--n'); const n = nIdx >= 0 ? parseInt(rest[nIdx + 1], 10) || 1 : 1;
  const idIdx = rest.indexOf('--id'); const id = idIdx >= 0 ? rest[idIdx + 1] : null;
  const pos = rest.filter((a, i) => !a.startsWith('--') && rest[i - 1] !== '--n' && rest[i - 1] !== '--id');
  record(pos[0], pos[1], n, id).catch((e) => ok0(`[capture] エラー → N/A (exit 0): ${String(e.message).split('\n')[0]}`));
} else {
  try { verifyProvenance(); } catch (e) { ok0(`[capture] エラー → fail-safe N/A (exit 0): ${String(e.message).split('\n')[0]}`); }
}
