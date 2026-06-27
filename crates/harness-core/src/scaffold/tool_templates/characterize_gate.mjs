#!/usr/bin/env node
// characterize_gate: 導出元カバレッジの決定論 floor ゲート。
//   spec.toml の全 [[acceptance]]/[[invariant]] が test スロットを「"true" でない実テストコマンド」へ
//   束縛しているかだけを確認する。これで既存 traceability_closed が実体化する。
//
// ★これは「完了/網羅」を主張しない floor ゲートである。
//   保証するのは「宣言済みの AC/INV 集合に対し test スロットが充填されている」ことだけで、
//   AC/INV の列挙自体が完全か(=穴が無いか)は本ゲートの保証外
//   (列挙完全性は research の deep-grilling / STPA-HAZOP 損失レンズと curated カタログの責務)。
//
// fail-safe: spec.toml が無い / AC・INV が1件も無い → exit 0 (N/A)。run を絶対に詰まらせない。
// 任意: CHARACTERIZE_GATE_RUN=1 で AC コマンドを実機実行し「characterize 時点で赤」を追加要求。
//
//   node bin/characterize_gate.mjs        (cwd=.harness。workflow.toml の cmd_exit_0 から)
//
// harness は gate cmd を cwd=.harness で実行するため bin/ 相対で指す(regression_gate と同規約)。

import { spawnSync } from 'node:child_process';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { homedir } from 'node:os';

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const HARNESS_DIR = dirname(SCRIPT_DIR);
const REPO_ROOT = dirname(HARNESS_DIR);
const SPEC = join(HARNESS_DIR, 'spec.toml');

const FORBIDDEN = ['true', '', 'tbd', 'todo', 'wip', 'fixme', '未定', '未確定', '要検討', '検討中', '対応予定', 'サンプル', 'ダミー', '仮置き'];

function spawnEnv() {
  const extra = [
    join(homedir(), '.cargo', 'bin'),
    process.env.LOCALAPPDATA ? join(process.env.LOCALAPPDATA, 'pnpm') : null,
    dirname(process.execPath),
  ].filter((d) => d && existsSync(d));
  const sep = process.platform === 'win32' ? ';' : ':';
  return { ...process.env, PATH: extra.join(sep) + sep + (process.env.PATH || '') };
}

// spec.toml から [[acceptance]]/[[invariant]] の {id, test} を抽出する小さな行パーサ。
// """ 複数行文字列はスキップ。test/id は単一行 `key = "..."` 形のみ拾う。
function parseSlots(text) {
  const lines = text.split('\n');
  const slots = [];
  let cur = null; // {kind, id, test}
  let inML = false;
  const flush = () => { if (cur) slots.push(cur); cur = null; };
  for (const raw of lines) {
    const line = raw.trim();
    const triples = (line.match(/"""/g) || []).length;
    if (inML) { if (triples % 2 === 1) inML = false; continue; }
    if (triples % 2 === 1) { inML = true; continue; }
    const sec = line.match(/^\[\[(acceptance|invariant)\]\]/);
    if (sec) { flush(); cur = { kind: sec[1], id: null, test: null }; continue; }
    if (/^\[\[?/.test(line)) { flush(); continue; } // 他セクション開始
    if (!cur) continue;
    const idm = line.match(/^id\s*=\s*"([^"]*)"/);
    if (idm) { cur.id = idm[1]; continue; }
    const tm = line.match(/^test\s*=\s*"([^"]*)"/);
    if (tm) { cur.test = tm[1]; continue; }
  }
  flush();
  return slots;
}

function isUnfilled(test) {
  if (test == null) return true;
  return FORBIDDEN.includes(test.trim().toLowerCase()) || FORBIDDEN.includes(test.trim());
}

function main() {
  if (!existsSync(SPEC)) { console.log('[characterize-gate] spec.toml 無し → N/A (exit 0)'); process.exit(0); }
  let slots;
  try { slots = parseSlots(readFileSync(SPEC, 'utf8')); }
  catch (e) { console.log(`[characterize-gate] spec.toml parse 失敗(${String(e.message).split('\n')[0]}) → fail-safe N/A (exit 0)`); process.exit(0); }
  if (slots.length === 0) { console.log('[characterize-gate] [[acceptance]]/[[invariant]] が1件も無い → N/A (exit 0)'); process.exit(0); }

  const unfilled = slots.filter((s) => isUnfilled(s.test));
  if (unfilled.length) {
    console.error('[characterize-gate] 束縛スロット未充填(test が "true"/空/禁止語) ── 実テストコマンドへ充填せよ:');
    for (const s of unfilled) console.error(`  ✗ ${s.kind}:${s.id || '(no id)'}  test=${JSON.stringify(s.test)}`);
    console.error('\n各 AC/INV の test を「それを担保する実テストの実コマンド」へ置換すること(導出元カバレッジの floor)。');
    process.exit(1);
  }

  if (process.env.CHARACTERIZE_GATE_RUN === '1') {
    const acs = slots.filter((s) => s.kind === 'acceptance');
    const green = [];
    for (const s of acs) {
      const r = spawnSync(s.test, { cwd: REPO_ROOT, shell: true, encoding: 'utf8', timeout: 900_000, maxBuffer: 64 * 1024 * 1024, env: spawnEnv() });
      if ((r.status ?? 1) === 0) green.push(s);
      process.stderr.write(`[characterize-gate] ${s.id}: ${(r.status ?? 1) === 0 ? '緑(NG)' : '赤(OK)'}\n`);
    }
    if (green.length) {
      console.error('[characterize-gate] characterize 時点で緑の AC(=正しい理由で赤になっていない):');
      for (const s of green) console.error(`  ✗ ${s.id}: ${s.test}`);
      process.exit(1);
    }
  }

  console.log(`[characterize-gate] OK ── 全 ${slots.length} AC/INV が非"true"の実テストへ束縛(floor)。`);
  console.log('  注: これは宣言済み上流への束縛 floor であり、AC/INV 列挙自体の完全性は未主張');
  console.log('  (列挙漏れは research の deep-grilling/STPA-HAZOP 損失レンズと curated カタログの責務)。');
  process.exit(0);
}

main();
