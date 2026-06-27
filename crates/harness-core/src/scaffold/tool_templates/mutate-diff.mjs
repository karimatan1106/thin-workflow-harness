#!/usr/bin/env node
// 差分 mutation(ハーネスの test ノード用・project 非依存)。
// baseRef との差分に生じる変異だけを測る。毎回フルは重いので「変更行だけ」。
//
//   node bin/mutate-diff.mjs [baseRef]          既定 audit: 変更行を cargo-mutants --in-diff(非ブロッキング evidence 用)
//   node bin/mutate-diff.mjs --gate [baseRef]   ★決定論ラチェットゲート: ベースライン比「新規/退行の非ledger生存ゼロ」で exit0
//   node bin/mutate-diff.mjs --update [baseRef]  現状の生存集合を baseline に上書き
//   node bin/mutate-diff.mjs --ratchet [baseRef] baseline を単調強化(殺せた生存は除く・新規は足さない)
//   MUTATE_DIFF_DRY=1 で実行せず対象だけ表示(audit のみ)
//
// ★--gate は「生存ゼロ到達」でなく「baseline 比で悪化させない(make-worse 禁止)」が teeth。
//   突合鍵 = file + 正規化変異テキスト(行番号不使用=ドリフト耐性)。equivalent は
//   state/equivalent_mutants.json に独立評価者(ADR-059)署名つきで登録した分だけ生存集合から控除。
//   出口は集合比較で壁時計非依存=決定論。budget(MUTATE_GATE_MAX_SECONDS)切れは常により寛容方向のみ。
//
// fail-safe: git 外 / 差分無 / 変更 .rs 無 / cargo-mutants 未導入 / baseline 空 → exit 0。run を詰まらせない。
// JS/TS は要コミット&ファイル→test 写像が要るため --gate の自動対象外(skill 05 step8 で mutate-isolated 手動+記録)。
//
// project 固有のパス/レイアウトは一切ハードコードしない(git とファイル探索だけで成立)。

import { execFileSync, spawnSync } from 'node:child_process';
import { writeFileSync, existsSync, readFileSync, rmSync, mkdtempSync } from 'node:fs';
import { join, dirname, resolve, delimiter } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';

const argv = process.argv.slice(2);
const MODE = argv.includes('--gate') ? 'gate' : argv.includes('--update') ? 'update' : argv.includes('--ratchet') ? 'ratchet' : 'audit';
const base = argv.find((a) => !a.startsWith('--')) || process.env.MUTATE_BASE || 'main';
const DRY = process.env.MUTATE_DIFF_DRY === '1';
const BUDGET_S = parseInt(process.env.MUTATE_GATE_MAX_SECONDS || '900', 10);

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
// state/ は .harness/state(本スクリプトは .harness/bin)。
const HARNESS_DIR = dirname(SCRIPT_DIR);
const BASELINE = join(HARNESS_DIR, 'state', 'mutation_baseline.json');
const LEDGER = join(HARNESS_DIR, 'state', 'equivalent_mutants.json');

const CARGO_BIN = join(process.env.HOME || process.env.USERPROFILE || '', '.cargo', 'bin');
const ENV = { ...process.env, PATH: `${CARGO_BIN}${delimiter}${process.env.PATH || ''}` };
const git = (args, o = {}) => execFileSync('git', args, { encoding: 'utf8', env: ENV, ...o });
const ok0 = (msg) => { console.log(msg); process.exit(0); };
const loadJson = (p, dflt) => { try { return JSON.parse(readFileSync(p, 'utf8')); } catch { return dflt; } };

let repoRoot;
try { repoRoot = git(['rev-parse', '--show-toplevel']).trim(); } catch { ok0('git リポジトリ外です → N/A (exit 0)'); }
const ROOT = resolve(repoRoot);

let changed = [];
try { changed = git(['diff', '--name-only', base], { cwd: repoRoot }).trim().split('\n').filter(Boolean); }
catch (e) { ok0(`git diff 失敗(base=${base}): ${String(e.message).split('\n')[0]} → N/A (exit 0)`); }

const rsFiles = changed.filter((f) => f.endsWith('.rs'));
const jsFiles = changed.filter((f) => /\.(ts|tsx|js|jsx|mjs|cjs)$/.test(f));
console.log(`mode=${MODE} base=${base}  変更 .rs=${rsFiles.length} / .ts・.js=${jsFiles.length}`);
if (jsFiles.length && MODE === 'gate') {
  console.log('[JS/TS] --gate の自動対象外(要コミット&file→test写像)。skill 05 step8 で mutate-isolated 手動測定+catalog_waivers/evidence に記録:');
  for (const f of jsFiles) console.log(`  ${f}`);
}

function findCargoRoot(absDir) {
  let d = absDir, firstCargo = null;
  for (;;) {
    const cargo = join(d, 'Cargo.toml');
    if (existsSync(cargo)) { if (!firstCargo) firstCargo = d; if (/^\s*\[workspace\]/m.test(readFileSync(cargo, 'utf8'))) return d; }
    const up = dirname(d);
    if (up === d || !d.startsWith(ROOT)) break;
    d = up;
  }
  return firstCargo;
}

const byRoot = new Map();
for (const f of rsFiles) {
  const root = findCargoRoot(dirname(resolve(repoRoot, f)));
  if (!root) continue;
  if (!byRoot.has(root)) byRoot.set(root, []);
  byRoot.get(root).push(f);
}

// ---- audit(既定・後方互換): 非ブロッキングに cargo-mutants --in-diff を流す ----
if (MODE === 'audit') {
  if (byRoot.size === 0) ok0('\n[Rust] 対象なし(変更 .rs に Cargo ワークスペースが無い → N/A)');
  try { execFileSync('cargo', ['mutants', '--version'], { stdio: 'ignore', env: ENV }); }
  catch { ok0('\n[Rust] cargo-mutants 未導入 → N/A (cargo install cargo-mutants)'); }
  let status = 0, idx = 0;
  for (const [root] of byRoot) {
    idx += 1;
    console.log(`\n[Rust] workspace=${root}  cargo mutants --in-diff (変更行のみ)`);
    const diffPath = join(tmpdir(), `mutate-diff-${process.pid}-${idx}.diff`);
    const diff = git(['diff', '--relative', base, '--', '*.rs'], { cwd: root });
    writeFileSync(diffPath, diff, { flag: 'wx' });
    if (DRY) { console.log(`  (DRY) cargo mutants --in-diff ${diffPath} --no-shuffle (cwd=${root})`); rmSync(diffPath, { force: true }); continue; }
    try { execFileSync('cargo', ['mutants', '--in-diff', diffPath, '--no-shuffle'], { cwd: root, stdio: 'inherit', env: ENV }); }
    catch (e) { status = e.status ?? 1; }
    finally { rmSync(diffPath, { force: true }); }
  }
  process.exit(status);
}

// ---- gate / update / ratchet: 構造化捕捉して集合で判定(決定論) ----
// missed.txt 行 "path:line:col: desc" → 鍵 "path::desc"(行番号/列を捨ててドリフト耐性)。
function normKey(line, root) {
  const m = line.match(/^(.+?\.rs):\d+(?::\d+)?:\s*(.+)$/);
  if (!m) return null;
  // path は workspace 相対(--relative diff)なので root 相対へ寄せる(rootは表示用に付ける)。
  return `${m[1]}::${m[2].trim()}`;
}

// 変更 .rs を持つ各 workspace で cargo-mutants を回し、missed 鍵集合を返す。budget 超過は lenient。
function collectMissed() {
  if (byRoot.size === 0) return { ran: false, reason: '変更 .rs に Cargo ワークスペース無し', keys: new Set() };
  try { execFileSync('cargo', ['mutants', '--version'], { stdio: 'ignore', env: ENV }); }
  catch { return { ran: false, reason: 'cargo-mutants 未導入', keys: new Set() }; }
  const keys = new Set();
  let truncated = false;
  let idx = 0;
  const startedAt = process.hrtime.bigint();
  for (const [root] of byRoot) {
    idx += 1;
    const elapsedS = Number(process.hrtime.bigint() - startedAt) / 1e9;
    if (elapsedS > BUDGET_S) { truncated = true; break; }
    const outDir = mkdtempSync(join(tmpdir(), `mutout-${process.pid}-${idx}-`));
    const diffPath = join(outDir, 'diff.patch');
    const diff = git(['diff', '--relative', base, '--', '*.rs'], { cwd: root });
    writeFileSync(diffPath, diff);
    const remainingS = Math.max(30, BUDGET_S - elapsedS);
    const r = spawnSync('cargo', ['mutants', '--in-diff', diffPath, '--no-shuffle', '--output', outDir], {
      cwd: root, encoding: 'utf8', env: ENV, timeout: Math.floor(remainingS * 1000), maxBuffer: 128 * 1024 * 1024,
    });
    if (r.error && r.error.code === 'ETIMEDOUT') { truncated = true; }
    // missed.txt を探す(cargo-mutants は <out>/mutants.out/ 配下に書く)。
    const candidates = [join(outDir, 'mutants.out', 'missed.txt'), join(outDir, 'missed.txt')];
    const mf = candidates.find((p) => existsSync(p));
    if (mf) {
      for (const line of readFileSync(mf, 'utf8').split('\n')) {
        const k = normKey(line.trim(), root);
        if (k) keys.add(k);
      }
    }
    try { rmSync(outDir, { recursive: true, force: true }); } catch {}
  }
  return { ran: true, truncated, keys };
}

const ledgerKeys = new Set((loadJson(LEDGER, []) || [])
  .filter((e) => e && e.evaluator === 'independent' && e.key && e.reason)
  .map((e) => e.key));

if (MODE === 'gate') {
  const { ran, reason, truncated, keys } = collectMissed();
  if (!ran) ok0(`\n[mutate-gate] ${reason} → N/A (exit 0)`);
  const baseObj = loadJson(BASELINE, {});
  const baseSet = new Set(Object.keys(baseObj || {}));
  if (baseSet.size === 0) {
    console.log(`\n[mutate-gate] baseline 空(未確立) → bootstrap: exit 0。teeth を効かせるには 'node bin/mutate-diff.mjs --update' を実行せよ。`);
    console.log(`  現状の missed=${keys.size}(うち ledger 控除前)。`);
    process.exit(0);
  }
  const novel = [...keys].filter((k) => !baseSet.has(k) && !ledgerKeys.has(k));
  console.error(`\n[mutate-gate] missed=${keys.size} / baseline=${baseSet.size} / ledger(equivalent)=${ledgerKeys.size}${truncated ? ' / ★budget超過につき計測済み集合のみで判定(lenient)' : ''}`);
  if (novel.length === 0) {
    ok0('[mutate-gate] OK ── baseline 比 新規/退行の非ledger生存ゼロ(make-worse なし)。');
  }
  console.error('[mutate-gate] baseline に無い新規/退行の生存変異体(=この挙動を契約するテストが無い):');
  for (const k of novel) console.error(`  ✗ ${k}`);
  console.error('\n対処(skill 05 step8): 各生存を殺すテストを AC/INV 紐付き(derived_from)で追加→再測。');
  console.error('  本物の equivalent なら独立評価者(ADR-059)署名つきで state/equivalent_mutants.json に登録:');
  console.error('  {"key":"<file::desc>","evaluator":"independent","reason":"<なぜ等価か>"}');
  console.error('  意図的に baseline を更新するなら --update / 強化のみなら --ratchet。');
  process.exit(1);
}

if (MODE === 'update' || MODE === 'ratchet') {
  const { ran, reason, keys } = collectMissed();
  if (!ran) ok0(`\n[mutate-${MODE}] ${reason} → 何も変更せず exit 0`);
  const baseObj = loadJson(BASELINE, {}) || {};
  let next;
  if (MODE === 'ratchet') {
    // 単調強化: 既存 baseline のうち今も生存している鍵だけ残す(殺せたものは除く)。新規は足さない。
    next = {};
    for (const k of Object.keys(baseObj)) if (keys.has(k)) next[k] = true;
  } else {
    next = {};
    for (const k of keys) next[k] = true;
  }
  writeFileSync(BASELINE, JSON.stringify(next, null, 2) + '\n');
  console.error(`[mutate-${MODE}] baseline ${MODE === 'ratchet' ? 'ratcheted' : 'written'} → ${BASELINE}  (生存 ${Object.keys(next).length} 件)`);
  process.exit(0);
}
