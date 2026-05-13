#![forbid(unsafe_code)]
//! 軽量ワークフローハーネス debug CLI ── core lib を呼ぶだけ。

fn main() {
    match thin_workflow_harness::cli::run() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
