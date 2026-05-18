//! thin-workflow-harness-core ── workflow runner core
//!
//! workflow.toml + spec.toml driven、event log + gate + runtime のみ。
//! CKG / daemon は harness binary 側で実装 (Phase 2 step 2 で harness-ckg crate 分離予定)。

#[cfg(not(windows))]
compile_error!("thin-workflow-harness-core is Windows-only");

pub mod detect;
pub mod event;
pub mod gate;
pub mod handlers;
pub mod handlers2;
pub mod handlers3;
pub mod handlers_advance;
pub mod handlers_init;
pub mod handlers_stats;
pub mod metrics;
pub mod paths;
pub mod questions;
pub mod runtime;
pub mod scaffold;
pub mod spec;
pub mod state;
pub mod status_view;
pub mod workflow;

pub use event::{Event, EventKind, FailedGate};
pub use gate::{eval_gate, GateCtx, GateResult, Question};
pub use questions::{append_answer, append_question, read_questions, QueuedQuestion};
pub use spec::{load_spec, Spec};
pub use state::{derive_state, State};
pub use workflow::{current_node, load_workflow, validate, GateSpec, Node, Workflow};
