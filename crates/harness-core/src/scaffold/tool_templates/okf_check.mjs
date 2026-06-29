#!/usr/bin/env node
// okf_check.mjs — Open Knowledge Format (OKF) v0.1 適合チェッカ (project 非依存・fail-safe)。
//
// 対象 = repo root の `docs/` 知識バンドル。harness gate は cwd=.harness で走るため `../docs` を既定とし、
// repo root から直接呼ばれた場合の `./docs` もフォールバックで見る。
//
// OKF v0.1 適合基準 (spec §5):
//   1. 非予約 .md は parseable な YAML frontmatter ブロックを持つ。
//   2. その frontmatter は非空の `type` を持つ。
//   3. 予約ファイル (index.md / log.md) は §6/§7 の構造に従う (index.md は frontmatter を持たない)。
//
// fail-safe: docs/ バンドル不在 / .md ゼロ → N/A で exit 0 (run を詰まらせない)。
// 既定 非ブロッキング: 違反があっても警告表示のみで exit 0。`OKF_STRICT=1` で違反時 exit 1 (強制)。
// ツール自身の例外も exit 0 に握る (gate を壊さない)。

import fs from "node:fs";
import path from "node:path";

const RESERVED = new Set(["index.md", "log.md"]);
const STRICT = process.env.OKF_STRICT === "1" || process.env.OKF_STRICT === "true";

function resolveBundle() {
  const candidates = [
    path.resolve(process.cwd(), "..", "docs"), // cwd=.harness (gate 既定)
    path.resolve(process.cwd(), "docs"), // repo root から直接
  ];
  for (const c of candidates) {
    try {
      if (fs.statSync(c).isDirectory()) return c;
    } catch {
      /* not found → 次へ */
    }
  }
  return null;
}

function walkMd(dir, out) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walkMd(p, out);
    else if (e.isFile() && e.name.endsWith(".md")) out.push(p);
  }
}

// 最小 frontmatter パーサ: 先頭 `---` 〜 次の `---` を走査し top-level key を拾う (YAML lib 非依存)。
function parseFrontmatter(text) {
  const lines = text.split(/\r?\n/);
  if (lines[0].trim() !== "---") return { has: false, fields: {} };
  let end = -1;
  for (let i = 1; i < lines.length; i++) {
    if (lines[i].trim() === "---") {
      end = i;
      break;
    }
  }
  if (end === -1) return { has: false, fields: {} }; // 未終端 = parseable でない
  const fields = {};
  for (let i = 1; i < end; i++) {
    const m = lines[i].match(/^([A-Za-z0-9_-]+)\s*:\s*(.*)$/);
    if (m) fields[m[1]] = m[2].trim();
  }
  return { has: true, fields };
}

function nonEmptyType(v) {
  if (v == null) return false;
  const t = String(v).replace(/^['"]|['"]$/g, "").trim();
  return t.length > 0;
}

function main() {
  const bundle = resolveBundle();
  if (!bundle) {
    console.log("[OKF] docs/ 知識バンドル不在 → N/A (fail-safe, exit 0)");
    return 0;
  }
  const files = [];
  walkMd(bundle, files);
  if (files.length === 0) {
    console.log(`[OKF] ${path.relative(process.cwd(), bundle)} に .md 無し → N/A (exit 0)`);
    return 0;
  }

  const violations = [];
  let concepts = 0;
  let conformant = 0;

  for (const f of files) {
    const rel = path.relative(bundle, f).split(path.sep).join("/");
    const base = path.basename(f);
    let text = "";
    try {
      text = fs.readFileSync(f, "utf8");
    } catch (e) {
      violations.push(`${rel}: 読み取り不可 (${e.message})`);
      continue;
    }
    const fm = parseFrontmatter(text);

    if (RESERVED.has(base)) {
      // §6: index.md は frontmatter を持たない。log.md は履歴ファイル (frontmatter 不要)。
      if (base === "index.md" && fm.has) {
        violations.push(`${rel}: 予約ファイル index.md は frontmatter を持つべきでない (OKF §6)`);
      }
      continue;
    }

    concepts++;
    if (!fm.has) {
      violations.push(`${rel}: parseable な YAML frontmatter ブロックが無い (OKF §5-1)`);
      continue;
    }
    if (!nonEmptyType(fm.fields.type)) {
      violations.push(`${rel}: 必須 \`type\` が無い/空 (OKF §5-2)`);
      continue;
    }
    conformant++;
  }

  const summary = {
    bundle: path.relative(process.cwd(), bundle).split(path.sep).join("/"),
    md_files: files.length,
    concept_docs: concepts,
    conformant,
    violations: violations.length,
    strict: STRICT,
  };
  console.log("[OKF] summary " + JSON.stringify(summary));

  if (violations.length === 0) {
    console.log(`[OK] OKF v0.1 conformant: 概念 ${concepts} 件すべて type 付き`);
    return 0;
  }

  console.log(`[OKF] ${violations.length} 件の非適合 (既定 非ブロッキング${STRICT ? " / OKF_STRICT=1 → 強制" : ""}):`);
  for (const v of violations) console.log(`  - ${v}`);
  return STRICT ? 1 : 0;
}

try {
  process.exit(main());
} catch (e) {
  console.log(`[OKF] チェック skip (fail-safe): ${e && e.message ? e.message : e}`);
  process.exit(0);
}
