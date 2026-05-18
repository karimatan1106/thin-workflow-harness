//! spec / workflow 系 gate。

use std::collections::HashSet;
use std::process::Command;

use super::{arg_str, GateCtx, GateResult};
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

