#!/usr/bin/env node
// 差分 mutation(ハーネスの test ノード用・project 非依存)。
// baseRef との差分に生じる変異だけを測る。毎回フルは重いので「変更行だけ」。
//
//   node .harness/bin/mutate-diff.mjs [baseRef]   (既定 baseRef=main)
//   MUTATE_DIFF_DRY=1 で実行せず対象だけ表示
//
// Rust: 変更 .rs を含む Cargo ワークスペースを自動検出し cargo-mutants --in-diff(変更行のみ)。
// JS/TS: 変更ファイルを列挙(プロジェクト側の mutation ツールを当てる前提・パス決め打ちしない)。
// 結果は `harness report-evidence mutation_diff '{...}'` に載せる(非ブロッキング)。
// exit: missed>0 で非0。ただしハーネスゲートは evidence_recorded なので advance は止めない。
//
// project 固有のパス/レイアウトは一切ハードコードしない(git とファイル探索だけで成立)。

import { execFileSync } from "node:child_process";
import { writeFileSync, existsSync, readFileSync, rmSync } from "node:fs";
import { join, dirname, resolve, delimiter } from "node:path";
import { tmpdir } from "node:os";

const base = process.argv[2] || process.env.MUTATE_BASE || "main";
const DRY = process.env.MUTATE_DIFF_DRY === "1";
// cargo/cargo-mutants は非対話シェルの PATH に無いことがあるため ~/.cargo/bin を補強
// (sibling の regression_gate.mjs と同じ理由)。
const CARGO_BIN = join(process.env.HOME || process.env.USERPROFILE || "", ".cargo", "bin");
const ENV = { ...process.env, PATH: `${CARGO_BIN}${delimiter}${process.env.PATH || ""}` };
// git/cargo はシェルを介さず引数配列で実行する(コマンドインジェクション防止)。
// base(argv/env 由来)をシェル文字列に展開しないことが要点。
const git = (args, o = {}) => execFileSync("git", args, { encoding: "utf8", env: ENV, ...o });

let repoRoot;
try {
  repoRoot = git(["rev-parse", "--show-toplevel"]).trim();
} catch {
  console.log("git リポジトリ外です");
  process.exit(2);
}
// git は "/" 区切り、Node resolve() は OS 区切り(Win は "\")。比較用に OS 正規化しておく。
const ROOT = resolve(repoRoot);

let changed = [];
try {
  changed = git(["diff", "--name-only", base], { cwd: repoRoot }).trim().split("\n").filter(Boolean);
} catch (e) {
  console.log(`git diff 失敗(base=${base}): ${String(e.message).split("\n")[0]}`);
  process.exit(2);
}

const rsFiles = changed.filter((f) => f.endsWith(".rs"));
const jsFiles = changed.filter((f) => /\.(ts|tsx|js|jsx|mjs|cjs)$/.test(f));
console.log(`base=${base}  変更 .rs=${rsFiles.length} / .ts・.js=${jsFiles.length}`);

// JS/TS は project の mutation ツール(Stryker / 自作ランナー等)に委ねる。パスは決め打ちしない。
if (jsFiles.length) {
  console.log("\n[JS/TS] 変更あり。プロジェクトの mutation ツールを下記に当てる:");
  for (const f of jsFiles) console.log(`  ${f}`);
}

// 変更 .rs を含む Cargo ワークスペース(または最寄 crate)を自動検出
function findCargoRoot(absDir) {
  let d = absDir;
  let firstCargo = null;
  for (;;) {
    const cargo = join(d, "Cargo.toml");
    if (existsSync(cargo)) {
      if (!firstCargo) firstCargo = d;
      if (/^\s*\[workspace\]/m.test(readFileSync(cargo, "utf8"))) return d;
    }
    const up = dirname(d);
    if (up === d || !d.startsWith(ROOT)) break; // fs ルート or repo 外まで来たら停止(OS正規化済で比較)
    d = up;
  }
  return firstCargo;
}

// 変更 .rs をワークスペースごとにまとめる
const byRoot = new Map();
for (const f of rsFiles) {
  const root = findCargoRoot(dirname(resolve(repoRoot, f)));
  if (!root) continue;
  if (!byRoot.has(root)) byRoot.set(root, []);
  byRoot.get(root).push(f);
}

if (byRoot.size === 0) {
  console.log("\n[Rust] 対象なし(変更 .rs に Cargo ワークスペースが無い → N/A)");
  process.exit(0);
}

// cargo-mutants 可用性を先に確認。cargo 自体が無い(ENOENT)も、サブコマンド未導入(exit 101)も
// ここで両方とも N/A に吸収する(後続の missed 非0 と取り違えない)。
try {
  execFileSync("cargo", ["mutants", "--version"], { stdio: "ignore", env: ENV });
} catch {
  console.log("\n[Rust] cargo-mutants 未導入 → N/A (cargo install cargo-mutants)");
  process.exit(0);
}

let status = 0;
let idx = 0;
for (const [root] of byRoot) {
  idx += 1;
  console.log(`\n[Rust] workspace=${root}  cargo mutants --in-diff (変更行のみ)`);
  const diffPath = join(tmpdir(), `mutate-diff-${process.pid}-${idx}.diff`);
  // --relative で workspace 相対パスの diff にする(cargo-mutants が解釈できる形)。cwd=root のため -C 不要。
  const diff = git(["diff", "--relative", base, "--", "*.rs"], { cwd: root });
  // flag 'wx': 既存(symlink 含む)があれば失敗させ、共有 tmpdir での symlink 追従を防ぐ。
  writeFileSync(diffPath, diff, { flag: "wx" });

  if (DRY) {
    console.log(`  (DRY) cargo mutants --in-diff ${diffPath} --no-shuffle  (cwd=${root})`);
    rmSync(diffPath, { force: true });
    continue;
  }
  try {
    execFileSync("cargo", ["mutants", "--in-diff", diffPath, "--no-shuffle"], {
      cwd: root,
      stdio: "inherit",
      env: ENV,
    });
  } catch (e) {
    status = e.status ?? 1; // missed があると非0(可用性は上で確認済み)
  } finally {
    rmSync(diffPath, { force: true }); // 一時 diff を残さない
  }
}
process.exit(status);
