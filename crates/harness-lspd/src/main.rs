#![forbid(unsafe_code)]
//! harness-lspd binary entry ── thin-workflow-harness-lspd::cli::run() を呼ぶだけ。

fn main() {
    match thin_workflow_harness_lspd::cli::run() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
