//! CLI サブコマンドのハンドラ（質問キュー系: ask / questions / answer / abandon）。

use crate::event::{append_event, read_events, EventKind};
use crate::handlers::state_for;
use crate::questions::{append_answer, append_question, next_question_id, read_questions, QueuedQuestion};
use crate::spec::load_spec;
use crate::workflow::current_node;
use crate::{handlers, paths};

/// `harness ask` ── worker が構造化質問をキューに積む。
pub fn cmd_ask(
    question: &str,
    options: &[String],
    header: Option<&str>,
    kind: Option<&str>,
    required: bool,
    run: Option<&str>,
) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let kind = kind.unwrap_or("clarification").to_string();
    let qid = next_question_id(&run_id)?;
    let q = QueuedQuestion {
        id: qid.clone(),
        kind: kind.clone(),
        question: question.to_string(),
        header: header.unwrap_or("質問").to_string(),
        options: options.to_vec(),
        required,
        context_ref: None,
    };
    append_question(&run_id, q)?;
    append_event(&run_id, EventKind::QuestionQueued { question_id: qid.clone(), kind, required })
        .map_err(|e| format!("question_queued 書込失敗: {e}"))?;
    println!("質問 {qid} をキューに積みました（required={required}）");
    Ok(())
}

/// `harness questions` ── 人間向けに保留中の質問を一覧する。
pub fn cmd_questions(run: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let qs = read_questions(&run_id)?;
    let mut pending: Vec<_> = qs.iter().filter(|q| !q.answered).collect();
    // required=true を先に
    pending.sort_by_key(|q| !q.required);
    if pending.is_empty() {
        println!("保留中の質問はありません");
        return Ok(());
    }
    for q in pending {
        let star = if q.required { "[必須]" } else { "[任意]" };
        println!("{} {} ({}) — {}", q.id, star, q.kind, q.header);
        println!("  {}", q.question);
        if q.options.is_empty() {
            println!("    (自由記述)");
        } else {
            println!("    選択肢（1 つ選択）:");
            for (i, opt) in q.options.iter().enumerate() {
                println!("      {i}) {opt}");
            }
        }
        if let Some(cr) = &q.context_ref {
            println!("    (関連: {cr})");
        }
        println!("  → harness answer {} <選択肢index または 自由記述>", q.id);
    }
    Ok(())
}

/// `harness answer` ── 人間が回答を記録する。
pub fn cmd_answer(question_id: &str, choice: &str, run: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let qs = read_questions(&run_id)?;
    let Some(q) = qs.iter().find(|q| q.id == question_id) else {
        return Err(format!("質問 '{question_id}' が見つからない"));
    };
    if q.answered {
        return Err(format!("質問 '{question_id}' は既に回答済み"));
    }
    // 選択肢 index なら本文に展開
    let answer_text = match choice.parse::<usize>() {
        Ok(i) if i < q.options.len() => q.options[i].clone(),
        _ => choice.to_string(),
    };
    // options 定義済みの質問は範囲外の回答を弾く（自由記述は無検証）。
    crate::questions::validate_answer(q, &answer_text)?;
    append_answer(&run_id, question_id, &answer_text)?;
    append_event(
        &run_id,
        EventKind::HumanAnswer { question_id: question_id.to_string(), answer: answer_text.clone() },
    )
    .map_err(|e| format!("human_answer 書込失敗: {e}"))?;
    println!("{question_id} に回答: {answer_text}");
    if q.kind == "clarification" {
        fill_open_question(q.context_ref.as_deref(), &answer_text)?;
    }
    Ok(())
}

/// clarification の回答を spec.toml の該当 [[open_question]] に埋め、`??` をクリアする。
/// （harness が config を書く例外 ── `docs/schemas.md` §1.1 / DESIGN.md §13.2）
fn fill_open_question(context_ref: Option<&str>, answer: &str) -> Result<(), String> {
    let sp_path = paths::spec_path();
    let Ok(spec) = load_spec(&sp_path) else {
        eprintln!("(警告) spec.toml が読めないため open_question 更新をスキップ");
        return Ok(());
    };
    // context_ref が open_question id にマッチするか、無ければ最初の未回答 open_question
    let target = spec
        .open_question
        .iter()
        .find(|oq| context_ref.is_some_and(|c| c == oq.id) && oq.answer.is_none())
        .or_else(|| spec.open_question.iter().find(|oq| oq.answer.is_none()));
    let Some(target) = target else {
        eprintln!("(警告) 対応する未回答の [[open_question]] が見つからない ── spec.toml は手動で更新してください");
        return Ok(());
    };
    let Ok(text) = std::fs::read_to_string(&sp_path) else {
        eprintln!("(警告) spec.toml が読めない");
        return Ok(());
    };
    // toml 値として再シリアライズすると配列順やコメントが崩れるので、テキスト編集で answer 行を追加する。
    let needle = format!("id = \"{}\"", target.id);
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    if let Some(pos) = lines.iter().position(|l| l.trim_start().starts_with(&needle)) {
        let esc = answer.replace('\\', "\\\\").replace('"', "\\\"");
        lines.insert(pos + 1, format!("answer = \"{esc}\""));
    }
    let joined = lines.join("\n").replace("??", "(解決済み)") + "\n";
    std::fs::write(&sp_path, joined).map_err(|e| format!("spec.toml 更新失敗: {e}"))?;
    println!("spec.toml の open_question '{}' に answer を記録、'??' をクリアしました", target.id);
    Ok(())
}

/// `harness abandon` ── run を放棄する（terminal イベント）。
pub fn cmd_abandon(run_id: &str, reason: Option<&str>) -> Result<(), String> {
    let wf = handlers::load_wf()?;
    let st = state_for(run_id, &wf)?;
    if st.abandoned {
        return Err(format!("run '{run_id}' は既に放棄済み"));
    }
    // 存在チェック（イベントが 1 つもなければエラー）
    if read_events(run_id)?.is_empty() {
        return Err(format!("run '{run_id}' が存在しない"));
    }
    let reason = reason.unwrap_or("(理由未記載)").to_string();
    append_event(run_id, EventKind::Abandon { reason: reason.clone() })
        .map_err(|e| format!("abandon 書込失敗: {e}"))?;
    println!("run {run_id} を放棄しました: {reason}");
    println!("注: worktree や作業ディレクトリの後始末（git worktree remove / git reset 等）は harness の仕事ではありません");
    Ok(())
}

/// `harness stuck` ── 現ノードで詰まったことを記録し人間にエスカレーションする。
/// `abandon`（run 全体の放棄）とは異なり、現ノード単位の node_aborted。
pub fn cmd_stuck(reason: &str, run: Option<&str>) -> Result<(), String> {
    let run_id = paths::resolve_run_id(run)?;
    let wf = handlers::load_wf()?;
    let st = state_for(&run_id, &wf)?;
    handlers::ensure_active(&st)?;
    if read_events(&run_id)?.is_empty() {
        return Err(format!("run '{run_id}' が存在しない"));
    }
    let node_id = current_node(&wf, &st).map(|n| n.id.clone());
    append_event(
        &run_id,
        EventKind::Stuck { reason: reason.to_string(), node_id: node_id.clone() },
    )
    .map_err(|e| format!("stuck 書込失敗: {e}"))?;
    let where_ = node_id.unwrap_or_else(|| "(完了済み)".to_string());
    println!("ノード {where_} を stuck（人間エスカレーション）にしました: {reason}");
    println!("注: 人間が判断し、back で戻す / abandon で放棄 / spec を見直す 等を選んでください");
    Ok(())
}
