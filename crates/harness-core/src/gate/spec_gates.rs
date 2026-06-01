//! spec / workflow 系 gate。

use std::collections::HashSet;
use std::process::Command;

use super::{arg_str, glob_paths, resolve, GateCtx, GateResult};
use crate::spec::Spec;
use crate::state::State;

fn run_cmd_ok(home: &std::path::Path, cmd: &str) -> bool {
    let mut c = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(cmd);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmd);
        c
    };
    c.current_dir(home);
    matches!(c.output(), Ok(o) if o.status.success())
}

/// `??` がどこかの text フィールドにあるか。
fn has_open_marker(spec: &Spec) -> bool {
    let mut texts = vec![spec.meta.intent.clone()];
    for r in &spec.requirement {
        texts.push(r.text.clone());
        if let Some(x) = &r.rationale {
            texts.push(x.clone());
        }
    }
    for a in &spec.acceptance {
        texts.push(a.text.clone());
    }
    for i in &spec.invariant {
        texts.push(i.text.clone());
    }
    for q in &spec.open_question {
        texts.push(q.text.clone());
    }
    texts.iter().any(|t| t.contains("??"))
}

pub(super) fn open_questions_zero(ctx: &GateCtx) -> GateResult {
    let Some(spec) = ctx.spec else {
        return GateResult::fail("spec.toml が無い");
    };
    if !spec.open_question.is_empty() {
        return GateResult::fail(format!("[[open_question]] が {} 件残っている", spec.open_question.len()));
    }
    if has_open_marker(spec) {
        return GateResult::fail("spec 本文に '??' が残っている");
    }
    GateResult::ok("open question なし、'??' なし")
}

pub(super) fn blast_radius_declared(ctx: &GateCtx) -> GateResult {
    let Some(spec) = ctx.spec else {
        return GateResult::fail("spec.toml が無い");
    };
    if spec.requirement.is_empty() {
        return GateResult::fail("requirement が 1 つも無い");
    }
    let missing: Vec<&str> = spec
        .requirement
        .iter()
        .filter(|r| r.files.is_empty())
        .map(|r| r.id.as_str())
        .collect();
    if missing.is_empty() {
        GateResult::ok(format!("全 {} 要件に影響ファイル宣言あり", spec.requirement.len()))
    } else {
        GateResult::fail(format!("影響ファイル未宣言の要件: {}", missing.join(", ")))
    }
}

pub(super) fn traceability_closed(state: &State, ctx: &GateCtx) -> GateResult {
    let Some(spec) = ctx.spec else {
        return GateResult::fail("spec.toml が無い");
    };
    // ① 各 F-NNN に実在 artifact ≥1 と exit 0 する test ≥1
    let declared_files: HashSet<String> = spec
        .requirement
        .iter()
        .flat_map(|r| r.files.iter().cloned())
        .collect();
    for r in &spec.requirement {
        let req_files: HashSet<&str> = r.files.iter().map(|s| s.as_str()).collect();
        // artifact が r.files のどれかに対応するか（パス末尾一致でゆるく）
        let has_artifact = state.artifacts.values().any(|p| {
            req_files.iter().any(|f| p == f || p.replace('\\', "/").ends_with(&f.replace('\\', "/")))
        });
        if !has_artifact {
            return GateResult::fail(format!("{} に紐づく artifact が無い", r.id));
        }
        if r.tests.is_empty() {
            return GateResult::fail(format!("{} に test コマンドが無い", r.id));
        }
        let ok_test = r.tests.iter().any(|c| run_cmd_ok(ctx.home, c));
        if !ok_test {
            return GateResult::fail(format!("{} の test が exit 0 しない", r.id));
        }
    }
    // ② orphan: 登録 artifact がどれかの F-NNN の files に含まれるか
    for (name, p) in &state.artifacts {
        let pn = p.replace('\\', "/");
        let belongs = declared_files
            .iter()
            .any(|f| p == f || pn.ends_with(&f.replace('\\', "/")));
        if !belongs {
            return GateResult::fail(format!("artifact '{name}' ({p}) がどの F-NNN にも紐づかない (orphan)"));
        }
    }
    GateResult::ok(format!("{} 要件すべてトレース閉じ", spec.requirement.len()))
}

pub(super) fn blast_radius_disjoint(args: &toml::Table, ctx: &GateCtx) -> GateResult {
    let Some(a) = arg_str(args, "node_a") else {
        return GateResult::fail("引数 node_a が無い");
    };
    let Some(b) = arg_str(args, "node_b") else {
        return GateResult::fail("引数 node_b が無い");
    };
    let Some(wf) = ctx.workflow else {
        return GateResult::fail("workflow が ctx に無い");
    };
    let Some(spec) = ctx.spec else {
        return GateResult::fail("spec.toml が無い");
    };
    let files_of = |node_id: &str| -> HashSet<String> {
        let mut set = HashSet::new();
        if let Some(n) = wf.node.iter().find(|n| n.id == node_id) {
            for fid in &n.serves {
                for r in &spec.requirement {
                    if &r.id == fid {
                        for f in &r.files {
                            set.insert(f.replace('\\', "/"));
                        }
                    }
                }
            }
        }
        set
    };
    let sa = files_of(a);
    let sb = files_of(b);
    let overlap: Vec<&String> = sa.intersection(&sb).collect();
    if overlap.is_empty() {
        GateResult::ok(format!("'{a}' と '{b}' の blast radius は互いに素"))
    } else {
        GateResult::fail(format!("'{a}' と '{b}' が共有: {:?}", overlap))
    }
}

/// ソースファイル中の `@spec <path>` コメントが参照する仕様書が実在するかを検証する。
///
/// プロジェクト規約 (sdd.md) では新規ソースに `@spec docs/specs/...` を付与するが、
/// harness はこれを検証していなかったため「実在しない仕様書を参照する」逃げが通っていた。
/// `path` (glob) でスキャン対象ソースを指定し、各ファイルから `@spec <ref>` を抽出、
/// ref が home 基準で実在しなければ fail。`@spec` が 1 つも無いのは許容 (規約は新規のみ)。
pub(super) fn spec_refs_exist(args: &toml::Table, ctx: &GateCtx) -> GateResult {
    let Some(p) = arg_str(args, "path") else {
        return GateResult::fail("引数 path (glob) が無い");
    };
    let hits = glob_paths(ctx.home, p);
    if hits.is_empty() {
        return GateResult::ok(format!("{p} 該当ソース 0 件 (@spec 検証対象なし)"));
    }
    let mut checked = 0usize;
    let mut missing: Vec<String> = Vec::new();
    for f in &hits {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        for refp in extract_spec_refs(&text) {
            checked += 1;
            // ref は repo root (home) 基準の相対パスとして解決する。
            if !resolve(ctx.home, &refp).is_file() {
                missing.push(format!("{} → {}", f.display(), refp));
            }
        }
    }
    if !missing.is_empty() {
        return GateResult::fail(format!(
            "@spec 参照先が実在しない {} 件: {}",
            missing.len(),
            missing.join("; ")
        ));
    }
    GateResult::ok(format!("@spec 参照 {checked} 件すべて実在 ({} ファイル走査)", hits.len()))
}

/// テキストから `@spec <ref>` の ref 部分を抽出する純関数。
/// `@spec docs/specs/x.md`、`* @spec docs/...`、`// @spec ...` 等に対応。
/// ref はホワイトスペース区切りの最初のトークン (末尾の句読点は剥がす)。
fn extract_spec_refs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(idx) = line.find("@spec") {
            let rest = line[idx + "@spec".len()..].trim_start();
            let token = rest.split_whitespace().next().unwrap_or("");
            let token = token.trim_end_matches([',', '.', ';', ')', ']', '"', '\'']);
            if !token.is_empty() {
                out.push(token.to_string());
            }
        }
    }
    out
}

pub(super) fn no_pending_required_questions(ctx: &GateCtx) -> GateResult {
    let pending: Vec<&str> = ctx
        .questions
        .iter()
        .filter(|q| q.required && !q.answered)
        .map(|q| q.id.as_str())
        .collect();
    if pending.is_empty() {
        GateResult::ok("未回答の必須質問なし")
    } else {
        GateResult::fail(format!("未回答の必須質問: {}", pending.join(", ")))
    }
}

