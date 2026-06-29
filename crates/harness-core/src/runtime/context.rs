//! `WorkerContext` の組み立て（`docs/worker-context.md` B2、決定論的）。
//!
//! skeleton では「コード本体・CKG 由来のアウトライン」は含まない ── blast-radius の
//! ファイルパス一覧だけ（CKG 未実装）。
//!
//! 静的 SYSTEM_PROMPT 本体は `system_prompt.rs` に分離（1024+ token を担保するための
//! 長文ゆえファイル分割）。ここでは import して `WorkerContext.system_prompt` に詰める。

use crate::event::{Event, EventKind};
use crate::handlers2::{eval_node_gates, RunCtx};
use crate::paths;
use crate::spec::Spec;
use crate::state::State;
use crate::workflow::{Node, Workflow};
use crate::runtime::system_prompt::SYSTEM_PROMPT;
use crate::runtime::worker::WorkerContext;

/// 常時渡される harness コマンド（`docs/worker-context.md` B1「渡すツール」）。
const ALWAYS_TOOLS: &[&str] = &[
    "status", "skill", "spec", "gates", "record-artifact", "report-evidence",
    "request-transition", "back", "ask",
];

/// ノード N の worker に渡す context バンドルを組み立てる（B2 の手順）。
///
/// `events` はその run のイベントログ全体（再 spawn 判定 ＝ 直前 advance_rejected の検出に使う）。
pub fn build_context(
    wf: &Workflow,
    node: &Node,
    st: &State,
    rc: &RunCtx,
    events: &[Event],
) -> WorkerContext {
    let node_header = format!("{} ({})", node.id, node.node_type());
    let skill_body = node
        .skill
        .as_deref()
        .and_then(|s| std::fs::read_to_string(paths::skill_path(s)).ok())
        // skill は OKF v0.1 知識バンドル(type: skill)として frontmatter を持ちうる。
        // worker プロンプトには本文だけ渡す(YAML が指示に混入しないよう先頭 frontmatter を剥がす)。
        .map(|c| strip_frontmatter(&c).to_string())
        .unwrap_or_default();
    let spec_slice = build_spec_slice(node, rc.spec.as_ref(), st);
    let compact_status = build_compact_status(wf, node, st, rc);
    let failed_gates = last_failed_gates(events);
    let mut tools: Vec<String> = ALWAYS_TOOLS.iter().map(|s| s.to_string()).collect();
    tools.extend(node.tools.iter().cloned());
    WorkerContext {
        system_prompt: SYSTEM_PROMPT.to_string(),
        node_header,
        skill_body,
        spec_slice,
        compact_status,
        failed_gates,
        tools,
    }
}

/// 先頭の YAML frontmatter ブロック(OKF v0.1)を剥がして本文だけ返す。
/// 先頭行が厳密に `---` の時のみ作動し、次の `---` 行の後ろを本文とする。
/// 終端 `---` が無ければ frontmatter でないとみなし元の文字列を返す(body 中の水平線 `---` は誤剥離しない)。
pub(crate) fn strip_frontmatter(s: &str) -> &str {
    let body = s.strip_prefix('\u{feff}').unwrap_or(s); // BOM 安全
    let after = match body.strip_prefix("---\n") {
        Some(a) => a,
        None => match body.strip_prefix("---\r\n") {
            Some(a) => a,
            None => return s,
        },
    };
    let mut offset = 0usize;
    for line in after.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(|c| c == '\r' || c == '\n');
        if trimmed == "---" {
            let rest = &after[offset + line.len()..];
            return rest.trim_start_matches(|c| c == '\r' || c == '\n');
        }
        offset += line.len();
    }
    s
}

/// serves する F-NNN とその AC・invariant・blast-radius ファイル一覧を抜き出す。
/// spec が無い研究ノード等では生 intent を使う。
fn build_spec_slice(node: &Node, spec: Option<&Spec>, st: &State) -> String {
    let Some(spec) = spec else {
        return format!("(spec 未確定) intent: {}", st.intent);
    };
    if node.serves.is_empty() {
        return format!("(このノードは特定 F-NNN を serve しない) intent: {}", st.intent);
    }
    let mut lines = Vec::new();
    for fid in &node.serves {
        let Some(req) = spec.requirement.iter().find(|r| &r.id == fid) else {
            lines.push(format!("{fid}: (spec.toml に未定義)"));
            continue;
        };
        lines.push(format!("{}: {}", req.id, req.text));
        if !req.files.is_empty() {
            lines.push(format!("  blast-radius files: {}", req.files.join(", ")));
        }
        for ac in spec.acceptance.iter().filter(|a| a.requirement == req.id) {
            lines.push(format!("  {} {} [test: {}]", ac.id, ac.text, ac.test));
        }
        for inv in &spec.invariant {
            lines.push(format!("  {} {} [test: {}]", inv.id, inv.text, inv.test));
        }
    }
    lines.join("\n")
}

/// コンパクト status（B1-(b)-4）── 現ノード X/Y、保留 gate 各 1 行、artifacts、evidence キー。
fn build_compact_status(wf: &Workflow, node: &Node, st: &State, rc: &RunCtx) -> String {
    let n = wf.nodes().len();
    let mut lines = vec![format!("node {}/{} {}", st.phase_index + 1, n, node.id)];
    for (name, r) in eval_node_gates(wf, node, st, rc) {
        let mark = if r.ok { "PASS" } else { "FAIL" };
        lines.push(format!("  gate {name}: {mark} — {}", r.note));
    }
    if st.artifacts.is_empty() {
        lines.push("artifacts: (なし)".to_string());
    } else {
        for (k, v) in &st.artifacts {
            lines.push(format!("artifact {k} -> {v}"));
        }
    }
    let ev: Vec<&str> = st.gate_evidence.keys().map(|s| s.as_str()).collect();
    lines.push(format!("evidence: {}", if ev.is_empty() { "(なし)".to_string() } else { ev.join(", ") }));
    lines.join("\n")
}

/// 最後の遷移/リセット以降の最新 `advance_rejected` の failed_gates を返す（再 spawn feedback）。
fn last_failed_gates(events: &[Event]) -> Vec<(String, String)> {
    let mut latest: Vec<(String, String)> = Vec::new();
    for ev in events {
        match &ev.kind {
            EventKind::Advance { .. } | EventKind::Back { .. } | EventKind::Reset => latest.clear(),
            EventKind::AdvanceRejected { failed_gates } => {
                latest = failed_gates.iter().map(|f| (f.gate.clone(), f.reason.clone())).collect();
            }
            _ => {}
        }
    }
    latest
}

#[cfg(test)]
mod frontmatter_tests {
    use super::strip_frontmatter;

    #[test]
    fn strips_leading_okf_frontmatter() {
        let s = "---\ntype: skill\ndescription: x\n---\n\n# skill: research\nbody";
        assert_eq!(strip_frontmatter(s), "# skill: research\nbody");
    }

    #[test]
    fn noop_when_no_frontmatter() {
        // 本文中の水平線 `---` を frontmatter と誤認しない(先頭行が `---` でないため)。
        let s = "# skill: research\n\n---\nsection break\n";
        assert_eq!(strip_frontmatter(s), s);
    }

    #[test]
    fn noop_when_unterminated() {
        let s = "---\ntype: skill\nno closing fence\n";
        assert_eq!(strip_frontmatter(s), s);
    }
}
