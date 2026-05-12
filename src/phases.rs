//! フェーズ定義（5 個）。

use crate::state::State;

pub struct Phase {
    pub name: &'static str,
    pub skill: &'static str,
    pub exit_gates: &'static [&'static str],
}

pub static PHASES: &[Phase] = &[
    Phase {
        name: "research",
        skill: "01-research.md",
        exit_gates: &["intent_recorded", "research_notes_recorded"],
    },
    Phase {
        name: "plan",
        skill: "02-plan.md",
        exit_gates: &["plan_artifact_exists", "plan_artifact_size_ok"],
    },
    Phase {
        name: "implement",
        skill: "03-implement.md",
        exit_gates: &["impl_artifacts_exist", "impl_artifacts_size_ok", "no_forbidden_words"],
    },
    Phase {
        name: "test",
        skill: "04-test.md",
        exit_gates: &["test_result_recorded_and_passing"],
    },
    Phase {
        name: "review",
        skill: "05-review.md",
        exit_gates: &["review_recorded"],
    },
];

/// 現在のフェーズ。done なら None。
pub fn current_phase(state: &State) -> Option<&'static Phase> {
    PHASES.get(state.phase_index)
}
