//! thin-workflow-harness core library。
//!
//! debug CLI（`src/cli.rs` / `src/main.rs`）と将来の runtime 層が共有する。
//! Phase 0 walking skeleton ── workflow.toml/spec.toml 駆動の決定論的状態機械。

pub mod ckg;
pub mod cli;
pub mod cli_dispatch;
pub mod detect;
pub mod event;
pub mod gate;
pub mod handlers;
pub mod handlers2;
pub mod handlers3;
pub mod handlers_advance;
pub mod handlers_closure;
pub mod handlers_find_symbol;
pub mod handlers_impacted;
pub mod handlers_init;
pub mod handlers_outline;
pub mod handlers_refs;
pub mod handlers_stats;
pub mod handlers_tested;
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
