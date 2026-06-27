#!/usr/bin/env node
// catalog_gate: omission を実際に削る唯一の外部錨。curated バグカタログ(domain rules JSON)の
//   各規則 from→to について、diff の追加行が規則の `from` に触れたら、その規則が「テストで殺せること
//   (=注入したら赤になるテストが在る)」を署名付き record で必須化する(silent skip 禁止=signed-empty-set)。
//
// teeth = forcing-function: プロジェクト固有の既知致命バグクラスに触れる変更は、その規則を意識的に
//   処理した record 無しには通さない。実際の注入検証は skill 05 step8 の mutation(MUTATE_DOMAIN=domain
//   operators)で行い、結果を waiver に署名付きで記録する。
//
// 規則ファイル(既定 .harness/domain_rules.json。env CATALOG_RULES_PATH で上書き)は
// [{"name","note","from"(正規表現),"to"}] の配列。**無ければ N/A で exit 0**(多くの project は持たない)。
// fail-safe: 規則無 / diff 無 / 触れた規則無 / parse 失敗 → exit 0。run を詰まらせない。
// .harness/ 自身(本ツール)は product code でないので走査除外。
//
//   node bin/catalog_gate.mjs [baseRef]   (既定 baseRef=main。cwd=.harness)

import { execFileSync } from 'node:child_process';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, delimiter } from 'node:path';

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const HARNESS_DIR = dirname(SCRIPT_DIR);
const REPO_ROOT = dirname(HARNESS_DIR);
const RULES = process.env.CATALOG_RULES_PATH
  ? join(REPO_ROOT, process.env.CATALOG_RULES_PATH)
  : join(HARNESS_DIR, 'domain_rules.json');
const WAIVERS = join(HARNESS_DIR, 'state', 'catalog_waivers.json');
const base = process.argv[2] || process.env.MUTATE_BASE || 'main';
const MAX_RULES = 5; // 有界(cost 安全)

const CARGO_BIN = join(process.env.HOME || process.env.USERPROFILE || '', '.cargo', 'bin');
const ENV = { ...process.env, PATH: `${CARGO_BIN}${delimiter}${process.env.PATH || ''}` };
const git = (args) => execFileSync('git', args, { encoding: 'utf8', env: ENV, cwd: REPO_ROOT });

// 走査除外: tooling とカタログ定義自身(規則文字列を含むため false positive 源)。
const EXCLUDE = [/^\.harness\//, /\.example\./, /domain_rules\.json$/];

function loadJson(p, dflt) { try { return JSON.parse(readFileSync(p, 'utf8')); } catch { return dflt; } }

function main() {
  if (!existsSync(RULES)) { console.log(`[catalog-gate] 規則ファイル無し(${RULES}) → N/A (exit 0)`); process.exit(0); }
  const rules = loadJson(RULES, null);
  if (!Array.isArray(rules) || rules.length === 0) { console.log('[catalog-gate] 規則 0 件 → N/A (exit 0)'); process.exit(0); }

  let diff;
  try { diff = git(['diff', '--unified=0', base]); }
  catch (e) { console.log(`[catalog-gate] git diff 失敗(base=${base}): ${String(e.message).split('\n')[0]} → fail-safe N/A (exit 0)`); process.exit(0); }
  if (!diff.trim()) { console.log('[catalog-gate] 差分なし → N/A (exit 0)'); process.exit(0); }

  const addedByFile = new Map();
  let curFile = null, included = false;
  for (const line of diff.split('\n')) {
    const fm = line.match(/^\+\+\+ b\/(.+)$/);
    if (fm) { curFile = fm[1]; included = !EXCLUDE.some((re) => re.test(curFile)); continue; }
    if (included && line.startsWith('+') && !line.startsWith('+++')) {
      if (!addedByFile.has(curFile)) addedByFile.set(curFile, []);
      addedByFile.get(curFile).push(line.slice(1));
    }
  }
  if (addedByFile.size === 0) { console.log('[catalog-gate] 対象ファイル(product code)の追加行なし → N/A (exit 0)'); process.exit(0); }

  const touched = [];
  for (const r of rules) {
    if (!r || !r.from) continue;
    let re; try { re = new RegExp(r.from); } catch { re = null; }
    const matchFiles = [];
    for (const [f, lines] of addedByFile) {
      if (lines.some((l) => (re ? re.test(l) : l.includes(r.from)))) matchFiles.push(f);
    }
    if (matchFiles.length) touched.push({ rule: r, files: matchFiles });
  }
  if (touched.length === 0) { console.log('[catalog-gate] curated 規則に触れる product 変更なし → OK (exit 0)'); process.exit(0); }

  const bounded = touched.slice(0, MAX_RULES);
  const waivers = loadJson(WAIVERS, []);
  const isWaived = (name) => Array.isArray(waivers) && waivers.some(
    (w) => w && w.rule === name && w.evaluator === 'independent' && (w.status === 'killed' || w.status === 'not_applicable') && w.reason);

  const unwaived = bounded.filter((t) => !isWaived(t.rule.name));
  console.error(`[catalog-gate] diff が触れた curated バグ規則: ${bounded.map((t) => t.rule.name).join(', ')}`);
  if (unwaived.length === 0) {
    console.log('[catalog-gate] OK ── 触れた規則すべてに署名付き record(killed/not_applicable)あり。');
    process.exit(0);
  }
  console.error('\n[catalog-gate] 以下の規則は既知の致命バグクラスに触れているが record が無い(silent skip 禁止):');
  for (const t of unwaived) {
    console.error(`  ✗ ${t.rule.name}: "${t.rule.note || ''}"  (touched: ${t.files.join(', ')})`);
  }
  console.error('\n対処: 各規則について skill 05-test step8 で MUTATE_DOMAIN 注入(from→to)が赤になるテストを書き、');
  console.error('  .harness/state/catalog_waivers.json に署名付き record を追加せよ(行番号不使用):');
  console.error('  {"rule":"<name>","evaluator":"independent","status":"killed","reason":"<どのテストが殺すか>"}');
  console.error('  本質的に注入無関係なら status:"not_applicable" + 独立評価者の理由(ADR-059)。');
  process.exit(1);
}

try { main(); } catch (e) { console.log(`[catalog-gate] 予期せぬエラー → fail-safe N/A (exit 0): ${String(e.message).split('\n')[0]}`); process.exit(0); }
