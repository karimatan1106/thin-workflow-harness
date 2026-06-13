#!/usr/bin/env node
// 回帰 gate (harness 同梱・言語非依存)。全テストスイートを実行し baseline と比較する。
//   各スイートで pass(成功数) >= floor-tol かつ fail(失敗数) <= ceiling+tol を満たせば exit 0。
//   既知の pre-existing 失敗は baseline に織込み、新規退行 (pass 減 / fail 増 / ビルド不能 / スイート崩壊)
//   だけを exit 1 で止める。harness の test ノード cmd_exit_0 が毎回 再実行する (自己申告でなく決定論)。
// 蓄積(積み上げ): テストを足して green にしたら `--ratchet` で floor を引上げる。baseline は
//   .harness/state/regression_baseline.json に git 追跡で永続し、プロジェクト全期間で単調強化される。
//
// 言語非依存の核: count 抽出は config 駆動。各スイートは regression_suites.json で
//   runner=プリセット名 (vitest/cargo/jest/pytest/go/dotnet/maven) を選ぶか、pass/fail の
//   抽出スペック {res:[正規表現...], agg:"last"|"sum"|"count"} を直接書いて任意 runner を追加する。
//   エンジンに per-runner 分岐は無い ── プリセットは下の PRESETS の「データ」にすぎない。
//
// モード: (既定)check / --update(再baseline上書き) / --ratchet(単調強化) / --selftest(プリセット自己検証)
//         / --parse-file <runner> <file>(デバッグ)。 修飾: --only a,b / --fast(slow除外)。
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { homedir } from 'node:os';

// cargo / pnpm はユーザプロファイル配下で非対話シェルの PATH に無いことがある。誰の環境から起動されても
// 解決できるよう既知の bin dir を PATH 先頭に補強する (テスト実行コマンド自体は各プロジェクトの責務)。
function spawnEnv() {
  const extra = [
    join(homedir(), '.cargo', 'bin'),
    process.env.LOCALAPPDATA ? join(process.env.LOCALAPPDATA, 'pnpm') : null,
    join(homedir(), '.local', 'share', 'pnpm'),
    dirname(process.execPath),
  ].filter((d) => d && existsSync(d));
  const sep = process.platform === 'win32' ? ';' : ':';
  return { ...process.env, PATH: extra.join(sep) + sep + (process.env.PATH || '') };
}

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const HARNESS_DIR = dirname(SCRIPT_DIR); // .harness
const REPO_ROOT = dirname(HARNESS_DIR);  // repo root (cwd 非依存に導出)
const CONFIG = join(HARNESS_DIR, 'regression_suites.json');
const BASELINE = join(HARNESS_DIR, 'state', 'regression_baseline.json');
const SUITE_TIMEOUT_MS = 900_000;

const stripAnsi = (s) => s.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '');

// 検証済みプリセット (各 runner の公式書式に基づく。fixture は --selftest 参照)。
//   pass/fail = { res: [行/トークン正規表現...], agg: last(各 res の最終マッチの群1) | sum(全マッチ群1の和) | count(マッチ数) }
//   フィールド値 = res 各々の寄与の総和。^ は multiline。先頭空白を許すため ^\s* を付ける。
const PRESETS = {
  // vitest: "Tests N passed | M failed" + ロード失敗は "Test Files F failed"。
  vitest: { pass: { res: ['^\\s*Tests\\s+.*?(\\d+)\\s+passed'], agg: 'last' },
            fail: { res: ['^\\s*Tests\\s+.*?(\\d+)\\s+failed', '^\\s*Test Files\\s+.*?(\\d+)\\s+failed'], agg: 'last' } },
  // cargo: "test result: ok. N passed; M failed" をバイナリ毎に合算。
  cargo: { pass: { res: ['test result:\\s*\\w+\\.\\s*(\\d+)\\s+passed'], agg: 'sum' },
           fail: { res: ['test result:\\s*\\w+\\.\\s*\\d+\\s+passed;\\s*(\\d+)\\s+failed'], agg: 'sum' } },
  // jest: "Tests: ... N passed" + collection エラーは "Test Suites: F failed"(Tests 行に出ない)。
  jest: { pass: { res: ['^\\s*Tests:\\s+.*?(\\d+)\\s+passed'], agg: 'last' },
          fail: { res: ['^\\s*Tests:\\s+.*?(\\d+)\\s+failed', '^\\s*Test Suites:\\s+.*?(\\d+)\\s+failed'], agg: 'last' } },
  // pytest: 中央寄せ "= N passed, M failed, E error in Xs ="。xpassed/xfailed を拾わぬよう語境界 (?<![\w-])。
  pytest: { pass: { res: ['(?<![\\w-])(\\d+)\\s+passed\\b'], agg: 'last' },
            fail: { res: ['(?<![\\w-])(\\d+)\\s+failed\\b', '(?<![\\w-])(\\d+)\\s+errors?\\b'], agg: 'last' } },
  // go: ネイティブ件数なし → "--- PASS:"/"--- FAIL:" 行を数える。**`go test -v ./...` 必須**(非 -v は PASS 行が出ない)。
  go: { pass: { res: ['^\\s*--- PASS:'], agg: 'count' },
        fail: { res: ['^\\s*--- FAIL:'], agg: 'count' } },
  // dotnet (VSTest): "Passed! - Failed: F, Passed: P, ..." を assembly 毎に合算。
  dotnet: { pass: { res: ['Passed:\\s*(\\d+)'], agg: 'sum' },
            fail: { res: ['Failed:\\s*(\\d+)'], agg: 'sum' } },
  // maven surefire: 集計行 "Tests run: N, Failures: F, Errors: E, Skipped: S"。pass は総数(skipped除外)を proxy に、fail=F+E。
  maven: { pass: { res: ['Tests run:\\s*(\\d+)'], agg: 'last' },
           fail: { res: ['Failures:\\s*(\\d+)', 'Errors:\\s*(\\d+)'], agg: 'last' } },
};

function applyField(spec, text) {
  let value = 0, matched = 0;
  for (const reStr of spec.res) {
    const all = [...text.matchAll(new RegExp(reStr, 'gm'))];
    matched += all.length;
    if (spec.agg === 'count') value += all.length;
    else if (spec.agg === 'sum') for (const m of all) value += parseInt(m[1], 10) || 0;
    else if (all.length) value += parseInt(all[all.length - 1][1], 10) || 0; // last
  }
  return { value, matched };
}

// 出力 → {pass, fail} | null。null = プリセット書式に合わない/ビルド失敗 (gate はラウドに落ちる)。
function extract(suite, out, status) {
  const passSpec = suite.pass || (PRESETS[suite.runner] && PRESETS[suite.runner].pass);
  const failSpec = suite.fail || (PRESETS[suite.runner] && PRESETS[suite.runner].fail);
  if (!passSpec || !failSpec) return null; // 未知 runner かつ inline spec 無し
  const text = stripAnsi(out);
  const p = applyField(passSpec, text), f = applyField(failSpec, text);
  if (p.matched === 0 && f.matched === 0) return null;          // 出力が書式に合わない
  if (p.value === 0 && f.value === 0 && status !== 0) return null; // 0/0 かつ非0 exit = collection/build error
  return { pass: p.value, fail: f.value };
}

function runSuite(s) {
  const r = spawnSync(s.cmd, { cwd: join(REPO_ROOT, s.cwd || '.'), shell: true, encoding: 'utf8', timeout: SUITE_TIMEOUT_MS, maxBuffer: 256 * 1024 * 1024, env: spawnEnv() });
  if (r.error && r.error.code === 'ETIMEDOUT') return { metrics: null, reason: `timeout >${SUITE_TIMEOUT_MS / 1000}s` };
  const out = (r.stdout || '') + '\n' + (r.stderr || '');
  const metrics = extract(s, out, r.status ?? 1);
  return { metrics, reason: metrics ? null : `サマリ解析不能 (exit ${r.status}) ── runner '${s.runner}' とコマンド/出力を確認 (ビルド失敗の可能性)` };
}

const loadJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

// 研究で得た各 runner の逐語サンプルでプリセットを自己検証 ([runner, output, expect_pass, expect_fail])。
function selftest() {
  const cases = [
    ['vitest', '      Tests  520 passed | 49 skipped (569)\n      Test Files  6 failed | 47 passed (53)', 520, 6],
    ['cargo', 'test result: ok. 21 passed; 0 failed; 0 ignored\ntest result: ok. 145 passed; 0 failed; 0 ignored', 166, 0],
    ['jest', 'Test Suites: 1 failed, 1 total\nTests:       0 total', 0, 1],
    ['jest', 'Test Suites: 2 passed, 2 total\nTests:       2 skipped, 2 todo, 2 passed, 6 total', 2, 0],
    ['pytest', '============= 2 failed, 1 passed, 1 error in 0.05s =============', 1, 3],
    ['pytest', '==== 1 xfailed, 1 xpassed, 2 passed in 0.04s ====', 2, 0],
    ['go', '--- PASS: TestHelloName (0.00s)\n--- PASS: TestHelloEmpty (0.00s)\nPASS\nok  example  0.3s', 2, 0],
    ['go', '--- FAIL: TestX (0.00s)\nFAIL\nexit status 1', 0, 1],
    ['dotnet', 'Passed!  - Failed:     0, Passed:     1, Skipped:     0, Total:     1\nFailed!  - Failed:     2, Passed:     3, Skipped:     0, Total:     5', 4, 2],
    ['maven', '[INFO] Results:\n[INFO]\n[INFO] Tests run: 268, Failures: 1, Errors: 0, Skipped: 3', 268, 1],
  ];
  let ok = true;
  for (const [runner, out, ep, ef] of cases) {
    const m = extract({ runner }, out, 0);
    const good = m && m.pass === ep && m.fail === ef;
    if (!good) ok = false;
    console.error(`${good ? '✓' : '✗'} ${runner}: got ${m ? `${m.pass}/${m.fail}` : 'null'} expect ${ep}/${ef}`);
  }
  console.error(ok ? '\n[selftest] OK ── 全プリセット検証済' : '\n[selftest] FAIL');
  process.exit(ok ? 0 : 1);
}

function main() {
  const args = process.argv.slice(2);
  if (args[0] === '--selftest') return selftest();
  if (args[0] === '--parse-file') {
    const m = extract({ runner: args[1] }, readFileSync(args[2], 'utf8'), 0);
    console.log(JSON.stringify(m));
    return process.exit(m ? 0 : 1);
  }
  const mode = args.includes('--update') ? 'update' : args.includes('--ratchet') ? 'ratchet' : 'check';
  let only = null;
  const oi = args.findIndex((a) => a === '--only' || a.startsWith('--only='));
  if (oi >= 0) only = (args[oi].includes('=') ? args[oi].split('=')[1] : args[oi + 1] || '').split(',').map((s) => s.trim()).filter(Boolean);
  if (!existsSync(CONFIG)) { console.error(`[regression-gate] ${CONFIG} が無い。スイート定義を作成せよ。`); process.exit(1); }
  const cfg = loadJson(CONFIG);
  const base = existsSync(BASELINE) ? loadJson(BASELINE) : {};
  if (mode === 'check' && !existsSync(BASELINE)) {
    console.error('[regression-gate] baseline 未確立。先に `node bin/regression_gate.mjs --update` を実行せよ。');
    process.exit(1);
  }
  const fast = args.includes('--fast');
  let items = only ? cfg.items.filter((s) => only.includes(s.name)) : cfg.items;
  if (fast) items = items.filter((s) => !s.slow);
  if (!items || items.length === 0) { console.error('[regression-gate] 実行対象スイートが無い (config 空 / 絞り込み過多)'); process.exit(1); }

  const results = [];
  for (const s of items) {
    process.stderr.write(`[regression-gate] running ${s.name} (${s.cmd})...\n`);
    results.push({ suite: s, ...runSuite(s) });
  }

  if (mode === 'update' || mode === 'ratchet') {
    const next = (mode === 'ratchet' || only) ? { ...base } : {};
    let failed = false;
    for (const { suite, metrics, reason } of results) {
      if (!metrics) { console.error(`[regression-gate] ${suite.name}: 計測失敗 (${reason}) ── baseline に含めない`); failed = true; continue; }
      const prev = base[suite.name];
      next[suite.name] = (mode === 'ratchet' && prev)
        ? { pass: Math.max(prev.pass, metrics.pass), fail: Math.min(prev.fail, metrics.fail) }
        : { pass: metrics.pass, fail: metrics.fail };
      console.error(`[regression-gate] ${suite.name}: pass=${metrics.pass} fail=${metrics.fail}` + (prev ? ` (prev ${prev.pass}/${prev.fail})` : ''));
    }
    writeFileSync(BASELINE, JSON.stringify(next, null, 2) + '\n');
    console.error(`[regression-gate] baseline ${mode === 'ratchet' ? 'ratcheted' : 'written'} → ${BASELINE}`);
    process.exit(failed ? 1 : 0);
  }

  let regressed = false;
  const rows = [];
  for (const { suite, metrics, reason } of results) {
    const b = base[suite.name];
    if (!metrics) { regressed = true; rows.push(`✗ ${suite.name}: ${reason}`); continue; }
    if (!b) { regressed = true; rows.push(`✗ ${suite.name}: baseline エントリ無し (--update 要)`); continue; }
    const tol = suite.tol || 0, floor = b.pass - tol, ceil = b.fail + tol, tn = tol ? `±${tol}` : '';
    if (metrics.pass >= floor && metrics.fail <= ceil) {
      rows.push(`✓ ${suite.name}: pass=${metrics.pass}(≥${b.pass}${tn}) fail=${metrics.fail}(≤${b.fail}${tn})`);
    } else {
      regressed = true;
      const why = [];
      if (metrics.pass < floor) why.push(`pass ${metrics.pass} < floor ${floor} (${floor - metrics.pass} 件 消失/退行)`);
      if (metrics.fail > ceil) why.push(`fail ${metrics.fail} > ceiling ${ceil} (新規失敗 ${metrics.fail - ceil} 件)`);
      rows.push(`✗ ${suite.name}: ${why.join(' / ')}`);
    }
  }
  console.error('\n[regression-gate] 結果 (baseline 比 新規失敗ゼロ判定):');
  for (const r of rows) console.error('  ' + r);
  if (regressed) {
    console.error('\n[regression-gate] REGRESSION 検出 ── implement に戻して修正、または意図的変更なら baseline を --update/--ratchet で更新せよ。');
    process.exit(1);
  }
  console.error('\n[regression-gate] OK ── 全スイート baseline 維持/改善。');
  process.exit(0);
}

main();
