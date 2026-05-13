//! Worker 抽象 ── ノード 1 つを担当する主体（Phase 1+ では生 API、Phase 1 skeleton ではスクリプト）。
//!
//! `trait Worker` は `act(&self, ctx) -> Vec<WorkerAction>`。harness はこの戻り値を
//! 「対応する `harness` コマンド相当の操作」として適用する（`docs/worker-context.md` B3）。

use std::cell::RefCell;

use crate::runtime::script::Step;

/// worker が harness に出す操作（`harness` サブコマンド／ツールに 1:1 対応）。
#[derive(Debug, Clone)]
pub enum WorkerAction {
    /// `create_file` ── スクリプト worker がアウトプットファイルを生成する（実 worker なら edit ツール経由）。
    CreateFile { path: String, content: String },
    /// `edit_file` / `write_file` ── ファイル編集（インターセプタが blast radius を強制）。
    EditFile { path: String, content: String },
    /// `run_command` ── コマンド実行（インターセプタが cmd_allowlist を強制）。
    RunCommand { cmd: String },
    /// `harness record-artifact <name> <path>`。
    RecordArtifact { name: String, path: String },
    /// `harness report-evidence <gate> <json>`。
    ReportEvidence { gate: String, json: String },
    /// `harness request-transition`（= 出口 gate 評価 → advance）。
    RequestTransition,
    /// `harness back <reason>`。
    #[allow(dead_code)]
    Back { reason: String },
    /// `harness ask <question> [--option ...] [--required]`。
    Ask { question: String, options: Vec<String>, required: bool },
    /// `harness stuck <reason>` ── 自己申告で人間にエスカレ（`docs/operations.md` §1）。
    Stuck { reason: String },
}

/// ノード 1 つを担当する主体。
pub trait Worker {
    /// 与えられた context（harness が組み立てた最小バンドル）に対し、出す操作の列を返す。
    fn act(&self, ctx: &WorkerContext) -> Vec<WorkerAction>;
}

/// harness が worker に渡す context バンドル（`docs/worker-context.md` B1）。
///
/// skeleton では「コード本体・CKG 由来のアウトライン」は含まない ── blast-radius の
/// ファイルパス一覧だけ（CKG 未実装、`docs/worker-context.md` B1-(b)-3 の代替）。
#[derive(Debug, Clone, Default)]
pub struct WorkerContext {
    /// 静的 system prompt の sketch（worker 間でほぼ不変、prompt cache 対象）。
    pub system_prompt: String,
    /// ノードヘッダ（id ＋ 種別）。
    pub node_header: String,
    /// `skills/<N.skill>` の本文（無ければ空）。
    pub skill_body: String,
    /// spec スライス ── serves する F-NNN / AC-N / invariant / blast-radius ファイル一覧。
    pub spec_slice: String,
    /// コンパクト status ── 現ノード X/Y、保留 gate 各 1 行、artifacts、evidence キー。
    pub compact_status: String,
    /// 再 spawn（直前 advance_rejected）時のみ ── failed gate の (gate, reason) 列。空なら初回 spawn。
    pub failed_gates: Vec<(String, String)>,
    /// 渡されるツール名（`workflow.toml` の `tools` ＋ 常時 harness コマンド）。
    pub tools: Vec<String>,
}

impl WorkerContext {
    /// 再 spawn かどうか（feedback が付いているか）。
    pub fn is_respawn(&self) -> bool {
        !self.failed_gates.is_empty()
    }
}

/// スクリプト（TOML）を replay する決定論的 worker（API コスト・非決定性なし）。
///
/// 多数の spawn 済み LLM worker の代役。保持するのは「スクリプトカーソル」だけで、
/// ノード間の推論は一切持ち越さない ── fresh-context プロパティはカーソルではなく
/// 「`WorkerContext` をノードごとに組み直す」ことで担保される（runtime/mod.rs 参照）。
pub struct ScriptedWorker {
    steps: Vec<Step>,
    cursor: RefCell<usize>,
}

impl ScriptedWorker {
    pub fn new(steps: Vec<Step>) -> Self {
        ScriptedWorker { steps, cursor: RefCell::new(0) }
    }

    /// まだ消費していない step のうち、現ノード id にマッチする最初のものを返す。
    /// 見つかればカーソルをその直後に進める。無ければ None（runtime 側で stuck 扱い）。
    fn take_step(&self, node_id: &str) -> Option<Step> {
        let mut cur = self.cursor.borrow_mut();
        let mut i = *cur;
        while i < self.steps.len() {
            if self.steps[i].node == node_id {
                *cur = i + 1;
                return Some(self.steps[i].clone());
            }
            i += 1;
        }
        None
    }
}

impl Worker for ScriptedWorker {
    fn act(&self, ctx: &WorkerContext) -> Vec<WorkerAction> {
        // node_header は "id (type)" 形式。先頭トークンが node id。
        let node_id = ctx.node_header.split_whitespace().next().unwrap_or("");
        match self.take_step(node_id) {
            Some(step) => step.actions,
            None => vec![WorkerAction::Stuck {
                reason: format!("スクリプトに node '{node_id}' に対応する未消費 step が無い"),
            }],
        }
    }
}
