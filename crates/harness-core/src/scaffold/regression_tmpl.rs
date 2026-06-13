//! `regression_suites.json` を検出結果から生成する。回帰 gate (`bin/regression_gate.mjs`) の対象 SSOT。
//! 検出言語から runner プリセットと既定テストコマンドを 1 スイート分だけ埋める (モノレポは items に追記)。

use crate::detect::DetectedProject;

pub fn render(d: &DetectedProject) -> String {
    let (runner, default_cmd) = runner_for(d.lang.as_deref());
    let cmd = d.full_suite.as_deref().or(d.test.as_deref()).unwrap_or(default_cmd);
    format!(
        r#"{{
  "_doc": "回帰 gate 対象スイート (SSOT)。test ノードの cmd_exit_0 が bin/regression_gate.mjs 経由で全 items を実機実行し state/regression_baseline.json と比較、各スイートで pass>=floor-tol かつ fail<=ceiling+tol を満たさなければ exit!=0 (baseline 比 新規失敗ゼロを強制)。各 item: name / cwd(repo root 相対) / cmd / runner(検証済プリセット: vitest|cargo|jest|pytest|go|dotnet|maven) / tol(flaky 許容幅, 既定0) / slow(--fast で除外)。プリセットに無い runner は runner を省き pass/fail に抽出スペック {{res:[正規表現...], agg:last|sum|count}} を直書きすれば config だけで任意言語に対応 (_examples の custom を参照)。再baseline: node bin/regression_gate.mjs --update(上書き) / --ratchet(floor 引上げ=蓄積)。プリセット検証: --selftest。",
  "items": [
    {{ "name": "unit", "cwd": ".", "runner": "{runner}", "cmd": "{cmd}", "tol": 0 }}
  ],
  "_examples": [
    {{ "name": "jest-pkg", "cwd": "packages/foo", "runner": "jest", "cmd": "pnpm exec jest --ci=false", "tol": 0, "slow": true }},
    {{ "name": "custom-any-runner", "cwd": ".", "cmd": "make test", "pass": {{ "res": ["(\\d+) ok"], "agg": "last" }}, "fail": {{ "res": ["(\\d+) not ok"], "agg": "last" }} }}
  ]
}}
"#,
        runner = runner,
        cmd = json_escape(cmd),
    )
}

/// 検出言語 → (runner プリセット名, 既定テストコマンド)。未検出は配置失敗で気づけるプレースホルダ。
fn runner_for(lang: Option<&str>) -> (&'static str, &'static str) {
    match lang {
        Some("rust") => ("cargo", "cargo test"),
        Some("typescript") | Some("javascript") | Some("node") => ("vitest", "pnpm exec vitest run"),
        Some("python") => ("pytest", "pytest --color=no -q"),
        Some("go") => ("go", "go test -v ./..."),
        _ => (
            "vitest",
            "false # configure: regression_suites.json の cmd と runner を設定 (vitest|cargo|jest|pytest|go|dotnet|maven、または pass/fail スペック直書き)",
        ),
    }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
