// preservation_lib: 挙動保存(rehost/migration)トラックの共有エンジン。oracle = 旧システムの実挙動。
//
// ★設計の cardinal sin 回避: capture / 比較で「強制ソート・blanket volatile drop」をしない。
//   それらは保存対象(class F collation/SORT 順)を壊し、腐った golden に全 differential が一致する
//   false-green を量産する。等価は **equivalence.json の per-field 宣言 opt-in** に後置し、order は
//   positional 既定(multiset は per-field opt-in)。各規則は scope(field/picture-range/byte-window)必須・
//   scope 無/blanket regex は reject。
//
// 公開: loadEquivalence(path) / compareUnderEquivalence(oldVal,newVal,policy) -> divergence[] /
//       rawCaptureFetch(opts) / contentHash(val,policy) / repoRootFrom(scriptUrl)
//
// 全関数は throw しない設計を心がけ、上位ゲートが fail-safe(N/A->exit0)を保てるようにする。

import { readFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { dirname } from 'node:path';

export function repoRootFrom(scriptUrl) {
  const scriptDir = dirname(fileURLToPath(scriptUrl)); // .../.harness/bin
  const harnessDir = dirname(scriptDir);               // .../.harness
  return { scriptDir, harnessDir, repoRoot: dirname(harnessDir) };
}

export function loadJsonSafe(p, dflt) {
  try { return JSON.parse(readFileSync(p, 'utf8')); } catch { return dflt; }
}

// equivalence.json: { rules: [ {id, field(JSONPath接頭辞 or 正規表現?いいえ=完全/接頭辞のみ), kind, ...scope} ] }
// kind: "volatile"(比較から除外) / "tolerance"(数値 abs/rel) / "transcode"(文字写像) / "order:multiset"(配列を集合扱い)
// ★scope 必須: field(具体パス or パス接頭辞)。"*" 等の blanket は reject(規則を無効化し warn)。
export function loadEquivalence(p) {
  const raw = loadJsonSafe(p, null);
  if (!raw || !Array.isArray(raw.rules)) return { rules: [], warnings: raw ? ['equivalence.json に rules 配列が無い'] : [] };
  const rules = [], warnings = [];
  for (const r of raw.rules) {
    if (!r || !r.field || !r.kind) { warnings.push(`規則に field/kind 欠落: ${JSON.stringify(r)}`); continue; }
    if (r.field === '*' || r.field === '' || r.field === '.*') { warnings.push(`blanket scope は reject: ${r.id || r.field}`); continue; }
    rules.push(r);
  }
  return { rules, warnings };
}

// path(例 "data.0.marketType")が規則 field(完全一致 or 接頭辞 "data." )にマッチするか。
function ruleMatches(rule, path) {
  const f = rule.field;
  if (f.endsWith('.')) return path === f.slice(0, -1) || path.startsWith(f);
  if (f.endsWith('[]')) { const base = f.slice(0, -2); return path === base || path.startsWith(base + '.'); }
  return path === f;
}
function ruleFor(rules, path, kind) {
  return rules.find((r) => r.kind === kind && ruleMatches(r, path));
}

function isObj(v) { return v && typeof v === 'object' && !Array.isArray(v); }

// 全 divergence を安定アドレス(JSON path)付きで列挙する。throw しない。
// policy = { rules } (loadEquivalence の戻り)。touched = 各規則が吸収した divergence 数を集計。
export function compareUnderEquivalence(oldVal, newVal, policy, opts = {}) {
  const rules = (policy && policy.rules) || [];
  const divergences = [];
  const absorbed = {}; // ruleId -> count
  const bump = (id) => { absorbed[id] = (absorbed[id] || 0) + 1; };

  function cmp(a, b, path) {
    // volatile: この path は比較しない(=吸収)。
    const vol = ruleFor(rules, path, 'volatile');
    if (vol) { if (a !== undefined || b !== undefined) bump(vol.id || vol.field); return; }
    // tolerance: 数値の許容差で吸収。
    const tol = ruleFor(rules, path, 'tolerance');
    if (tol && typeof a === 'number' && typeof b === 'number') {
      const absd = Math.abs(a - b);
      const rel = absd / (Math.max(Math.abs(a), Math.abs(b)) || 1);
      if (absd <= (tol.abs ?? 0) || rel <= (tol.rel ?? 0)) { if (a !== b) bump(tol.id || tol.field); return; }
    }
    // transcode: 文字写像で吸収(map: {from:to} を a に適用してから比較)。
    const tc = ruleFor(rules, path, 'transcode');
    if (tc && typeof a === 'string' && typeof b === 'string' && tc.map) {
      let ta = a; for (const [k, v] of Object.entries(tc.map)) ta = ta.split(k).join(v);
      if (ta === b) { if (a !== b) bump(tc.id || tc.field); return; }
    }
    // range: 制約付き非決定 field の軽量 admissible-set(数値域)。旧値(a)は非決定なので比較せず、
    //   ★新値(b)が宣言した [min,max] に入るかだけ検証。域内=吸収 / 域外=divergence(garbage 捕捉)。
    const rng = ruleFor(rules, path, 'range');
    if (rng) {
      if (typeof b === 'number' && (rng.min === undefined || b >= rng.min) && (rng.max === undefined || b <= rng.max)) {
        bump(rng.id || rng.field); return;
      }
      divergences.push({ path: path || '(root)', old: a, new: b, rule: 'range', note: `new ${JSON.stringify(b)} ∉ [${rng.min ?? '-inf'},${rng.max ?? '+inf'}]` });
      return;
    }
    // enum: 制約付き非決定 field の軽量 admissible-set(許容集合)。新値(b)が allowed に入るかだけ検証。
    const en = ruleFor(rules, path, 'enum');
    if (en && Array.isArray(en.allowed)) {
      if (en.allowed.some((v) => JSON.stringify(v) === JSON.stringify(b))) { bump(en.id || en.field); return; }
      divergences.push({ path: path || '(root)', old: a, new: b, rule: 'enum', note: `new ${JSON.stringify(b)} ∉ allowed` });
      return;
    }
    // 型不一致 or プリミティブ不一致
    if (typeof a !== typeof b || a === null || b === null || typeof a !== 'object') {
      if (a !== b) divergences.push({ path: path || '(root)', old: a, new: b });
      return;
    }
    if (Array.isArray(a) !== Array.isArray(b)) { divergences.push({ path, old: a, new: b }); return; }
    if (Array.isArray(a)) {
      // order: positional 既定。per-field opt-in で multiset(ソートして比較)。
      const ord = ruleFor(rules, path, 'order:multiset') || ruleFor(rules, path, 'order');
      let aa = a, bb = b;
      if (ord && (ord.mode === 'multiset' || ord.kind === 'order:multiset')) {
        const key = (x) => JSON.stringify(x);
        aa = [...a].sort((x, y) => key(x) < key(y) ? -1 : 1);
        bb = [...b].sort((x, y) => key(x) < key(y) ? -1 : 1);
      }
      const n = Math.max(aa.length, bb.length);
      for (let i = 0; i < n; i++) cmp(aa[i], bb[i], `${path}[${i}]`);
      return;
    }
    // object: キー和集合を positional でなくキーで比較。
    const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
    for (const k of keys) cmp(a[k], b[k], path ? `${path}.${k}` : k);
  }
  try { cmp(oldVal, newVal, ''); } catch (e) { divergences.push({ path: '(compare-error)', old: String(e.message).split('\n')[0], new: null }); }
  return { divergences, absorbed, warnings: (policy && policy.warnings) || [] };
}

// 中和射影(volatile を除いた)上で content-hash を算出(golden の安定 signature)。
export function contentHash(val, policy) {
  const rules = (policy && policy.rules) || [];
  function strip(v, path) {
    if (ruleFor(rules, path, 'volatile')) return undefined;
    if (Array.isArray(v)) return v.map((x, i) => strip(x, `${path}[${i}]`));
    if (isObj(v)) { const o = {}; for (const k of Object.keys(v).sort()) { const s = strip(v[k], path ? `${path}.${k}` : k); if (s !== undefined) o[k] = s; } return o; }
    return v;
  }
  return createHash('sha256').update(JSON.stringify(strip(val, ''))).digest('hex').slice(0, 16);
}

// 旧/新システムへの I/O adapter。HTTP(GET)+ 認証ヘッダ注入。file/byte は呼び出し側で読む。
// ★normalize は一切しない(raw 取得)。status も返す(BUG3 のような status 差を捕えるため)。
export async function rawCaptureFetch({ base, path, headers = {}, timeoutMs = 15000 }) {
  const url = `${base.replace(/\/$/, '')}${path}`;
  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    const res = await fetch(url, { headers, signal: ctrl.signal });
    let body; const text = await res.text();
    try { body = JSON.parse(text); } catch { body = text; }
    return { ok: true, status: res.status, body };
  } catch (e) {
    return { ok: false, error: String(e.message).split('\n')[0] };
  } finally { clearTimeout(t); }
}
