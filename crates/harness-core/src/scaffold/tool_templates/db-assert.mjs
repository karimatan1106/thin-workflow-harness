#!/usr/bin/env node
// db-assert: read-HTTP に出ない write-path/state 欠陥(例: 移行で列が全 NULL 化)を捕えるための
//   DB/state oracle adapter。differential の非HTTP adapter として使う(skill から呼ぶ)。
//
// なぜ必要か: 移植脱落の一部は「read エンドポイントは値を忠実に射影するが、書き込み側(collector/ETL)が
//   壊れて DB の列が全 NULL」型。post-cutover の read-vs-read は旧新とも NULL を読み一致=false-green。
//   よって read 比較でなく **DB 不変条件**(例: count(col IS NULL WHERE should_be_set)=0)で oracle 化する。
//
// このテンプレは project 非依存の骨格。実際の接続/クエリは project 固有(DSN/ドライバ)なので、
//   .harness/db_assertions.json に [{ id, query, expect }] を宣言し、project 側で runner を配線する。
//   宣言が無ければ N/A で exit0(fail-safe・run を詰まらせない)。
//
//   node bin/db-assert.mjs   (cwd=.harness)

import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const harnessDir = dirname(dirname(fileURLToPath(import.meta.url)));
const SPEC = join(harnessDir, 'db_assertions.json');

if (!existsSync(SPEC)) {
  console.log('[db-assert] db_assertions.json 無し → N/A (exit 0)。');
  console.log('  使うとき: .harness/db_assertions.json に [{"id","query":"SELECT count(*) ...","expect":0}] を宣言し、');
  console.log('  project の DB ドライバで本 runner を実装/配線(DSN は env)。write-path 欠陥(列 NULL 化等)の oracle。');
  process.exit(0);
}

let spec;
try { spec = JSON.parse(readFileSync(SPEC, 'utf8')); } catch (e) {
  console.log(`[db-assert] db_assertions.json parse 失敗 → fail-safe N/A (exit 0): ${String(e.message).split('\n')[0]}`); process.exit(0);
}
if (!Array.isArray(spec) || spec.length === 0) { console.log('[db-assert] 宣言 0 件 → N/A (exit 0)'); process.exit(0); }

// ★ project 非依存テンプレでは実 DB 接続は持たない(ドライバが project 固有)。
//   宣言が在るのに runner 未配線 = 「検査すべき不変条件が宣言済みだが実行されていない」= 危険なので exit1 で気付かせる。
console.error(`[db-assert] db_assertions.json に ${spec.length} 件の不変条件が宣言されているが、`);
console.error('  project 固有の DB runner が未配線。下記を実装して各 query を実行し expect と突合せよ:');
for (const a of spec) console.error(`  - ${a.id}: ${a.query}  (expect ${JSON.stringify(a.expect)})`);
console.error('  実装するまで preservation の write-path oracle は塞がっていない(意図的に exit1 で可視化)。');
process.exit(1);
