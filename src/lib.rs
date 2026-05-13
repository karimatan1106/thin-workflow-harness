//! thin-workflow-harness core library。
//!
//! debug CLI（`src/cli.rs` / `src/main.rs`）と将来の runtime 層が共有する。
//! Phase 0 walking skeleton ── workflow.toml/spec.toml 駆動の決定論的状態機械。

pub mod cli;
pub mod event;
pub mod gate;
pub mod handlers;
pub mod handlers2;
pub mod paths;
pub mod spec;
pub mod state;
pub mod status_view;
pub mod workflow;

pub use event::{Event, EventKind, FailedGate};
pub use gate::{eval_gate, GateCtx, GateResult};
pub use spec::{load_spec, Spec};
pub use state::{derive_state, State};
pub use workflow::{current_node, load_workflow, validate, GateSpec, Node, Workflow};
