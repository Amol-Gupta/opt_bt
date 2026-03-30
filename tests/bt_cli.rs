use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn unique_workspace() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    std::env::temp_dir().join(format!("bt_cli_test_{}_{}", std::process::id(), nanos))
}

fn run_bt(args: &[&str], cwd: &PathBuf) -> std::process::Output {
    Command::new("cargo")
        .current_dir(cwd)
        .args(["run", "--bin", "bt", "--"])
        .args(args)
        .env("BT_ENGINE_PROFILE", "dev")
        .output()
        .expect("failed to run bt command")
}

#[test]
fn bt_workspace_and_project_init_scaffold() {
    let root = repo_root();
    let ws = unique_workspace();

    let out = run_bt(
        &[
            "workspace",
            "init",
            "--path",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(
        out.status.success(),
        "workspace init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_bt(
        &[
            "project",
            "init",
            "demo",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(
        out.status.success(),
        "project init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(ws.join(".bt/workspace.toml").exists());
    assert!(ws.join("projects/demo/bt.toml").exists());
    assert!(ws.join("projects/demo/strategy/Cargo.toml").exists());
    assert!(ws.join("projects/demo/strategy/src/demo.rs").exists());
    assert!(ws
        .join("projects/demo/generated/strategy_registry.rs")
        .exists());

    let _ = fs::remove_dir_all(ws);
}

#[test]
fn bt_one_command_run_from_scaffold_project() {
    let root = repo_root();
    let ws = unique_workspace();
    let data = root.join("sample_data/niftyIndex2024.sample.parquet");

    let out = run_bt(
        &[
            "workspace",
            "init",
            "--path",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "workspace init failed");

    let out = run_bt(
        &[
            "project",
            "init",
            "demo",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "project init failed");

    let out = run_bt(
        &[
            "run",
            "--project",
            "demo",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--strategy",
            "demo",
            "--data",
            data.to_string_lossy().as_ref(),
            "--start-date",
            "2024-01-01",
            "--end-date",
            "2024-01-31",
            "--params",
            "prob=0.0",
            "--params",
            "qty=1",
        ],
        &root,
    );

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("cache-server is required in cache-only mode"),
            "bt run failed for unexpected reason\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            stderr
        );

        let _ = fs::remove_dir_all(ws);
        return;
    }

    let backtests_root = ws.join("projects/demo/backtests");
    assert!(backtests_root.exists(), "expected backtests output folder");

    let mut entries: Vec<_> = fs::read_dir(&backtests_root)
        .expect("failed to read backtests dir")
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    let latest = entries
        .last()
        .expect("expected at least one backtest run folder");
    let run_dir = latest.path();

    let report_path = run_dir.join("report.json");
    let log_path = run_dir.join("engine.log");
    assert!(report_path.exists(), "expected report.json in run folder");
    assert!(log_path.exists(), "expected engine.log in run folder");

    let report = fs::read_to_string(report_path).expect("failed to read report.json");
    assert!(
        report.contains("\"metrics\""),
        "expected metrics block in report.json"
    );

    let _ = fs::remove_dir_all(ws);
}

#[test]
fn bt_list_strategies_discovers_project_strategy() {
    let root = repo_root();
    let ws = unique_workspace();

    let out = run_bt(
        &[
            "workspace",
            "init",
            "--path",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "workspace init failed");

    let out = run_bt(
        &[
            "project",
            "init",
            "demo",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "project init failed");

    let out = run_bt(
        &[
            "list-strategies",
            "--project",
            "demo",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--json",
        ],
        &root,
    );

    assert!(
        out.status.success(),
        "list-strategies failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("demo"),
        "expected project strategy id in discovery output"
    );

    let _ = fs::remove_dir_all(ws);
}

#[test]
fn bt_run_rejects_invalid_typed_param_from_strategy_metadata() {
    let root = repo_root();
    let ws = unique_workspace();
    let data = root.join("sample_data/niftyIndex2024.sample.parquet");

    let out = run_bt(
        &[
            "workspace",
            "init",
            "--path",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "workspace init failed");

    let out = run_bt(
        &[
            "project",
            "init",
            "demo",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "project init failed");

    let strategy_lib = ws.join("projects/demo/strategy/src/demo.rs");
    fs::write(
        &strategy_lib,
        r#"use opt_bt::common::context::Context;
use opt_bt::common::event::{FillEvent, MarketEvent, OrderEvent, SignalEvent};
use opt_bt::strategy::Strategy;

pub struct SampleStrategy;

impl Strategy for SampleStrategy {
    fn on_market_event(&mut self, _ctx: &mut Context, _event: &MarketEvent) {}
    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {}
    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {}
    fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {}
}

bt_strategy_sdk::inventory::submit! {
    bt_strategy_sdk::StrategyRegistration {
        metadata: bt_strategy_sdk::StrategyMetadata {
            id: "random",
            display_name: "Random",
            description: "Random strategy",
            parameters: &[
                bt_strategy_sdk::ParameterSpec {
                    name: "prob",
                    kind: "float",
                    required: true,
                    default_value: None,
                    description: "Probability",
                },
                bt_strategy_sdk::ParameterSpec {
                    name: "qty",
                    kind: "int",
                    required: false,
                    default_value: Some("1"),
                    description: "Quantity",
                },
                bt_strategy_sdk::ParameterSpec {
                    name: "seed",
                    kind: "int",
                    required: false,
                    default_value: Some("42"),
                    description: "Seed",
                }
            ],
        }
    }
}
"#,
    )
    .expect("failed to write strategy lib for typed metadata test");

    let out = run_bt(
        &[
            "run",
            "--project",
            "demo",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--strategy",
            "random",
            "--data",
            data.to_string_lossy().as_ref(),
            "--params",
            "prob=not_a_float",
        ],
        &root,
    );

    assert!(
        !out.status.success(),
        "bt run should fail when typed param is invalid\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("invalid value for parameter 'prob'"),
        "expected typed param validation error, got: {}",
        stderr
    );

    let _ = fs::remove_dir_all(ws);
}

#[test]
fn bt_list_strategies_fails_when_fallback_disabled_and_discovery_unavailable() {
    let root = repo_root();
    let ws = unique_workspace();

    let out = run_bt(
        &[
            "workspace",
            "init",
            "--path",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "workspace init failed");

    let out = run_bt(
        &[
            "project",
            "init",
            "demo",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "project init failed");

    let project_cfg = ws.join("projects/demo/bt.toml");
    let original = fs::read_to_string(&project_cfg).expect("failed to read bt.toml");
    let updated = original.replace("stage1_fallback = true", "stage1_fallback = false");
    fs::write(&project_cfg, updated).expect("failed to write bt.toml");

    let discovery_bin = ws.join("projects/demo/strategy/src/bin/bt_list_strategies.rs");
    fs::remove_file(&discovery_bin).expect("failed to remove strategy discovery bin");

    let out = run_bt(
        &[
            "list-strategies",
            "--project",
            "demo",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--json",
        ],
        &root,
    );

    assert!(
        !out.status.success(),
        "list-strategies should fail when discovery is unavailable and fallback is disabled\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("strategy discovery compile/run failed"),
        "expected discovery compile failure error, got: {}",
        stderr
    );

    let _ = fs::remove_dir_all(ws);
}

#[test]
fn bt_run_fails_when_discovery_compile_run_fails() {
    let root = repo_root();
    let ws = unique_workspace();
    let data = root.join("sample_data/niftyIndex2024.sample.parquet");

    let out = run_bt(
        &[
            "workspace",
            "init",
            "--path",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "workspace init failed");

    let out = run_bt(
        &[
            "project",
            "init",
            "demo",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "project init failed");

    let discovery_bin = ws.join("projects/demo/strategy/src/bin/bt_list_strategies.rs");
    fs::remove_file(&discovery_bin).expect("failed to remove strategy discovery bin");

    let out = run_bt(
        &[
            "run",
            "--project",
            "demo",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--strategy",
            "demo",
            "--data",
            data.to_string_lossy().as_ref(),
        ],
        &root,
    );

    assert!(
        !out.status.success(),
        "bt run should fail when discovery compile/run fails\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("strategy discovery compile/run failed"),
        "expected discovery compile failure error, got: {}",
        stderr
    );

    let _ = fs::remove_dir_all(ws);
}

#[test]
fn bt_mixed_mode_projects_stage2_succeeds_and_stage1_discovery_failure_errors() {
    let root = repo_root();
    let ws = unique_workspace();

    let out = run_bt(
        &[
            "workspace",
            "init",
            "--path",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "workspace init failed");

    let out = run_bt(
        &[
            "project",
            "init",
            "stage2",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "stage2 project init failed");

    let out = run_bt(
        &[
            "project",
            "init",
            "stage1",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--force",
        ],
        &root,
    );
    assert!(out.status.success(), "stage1 project init failed");

    let stage1_discovery_bin = ws.join("projects/stage1/strategy/src/bin/bt_list_strategies.rs");
    fs::remove_file(&stage1_discovery_bin).expect("failed to remove stage1 strategy discovery bin");

    let out_stage2 = run_bt(
        &[
            "list-strategies",
            "--project",
            "stage2",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--json",
        ],
        &root,
    );
    assert!(
        out_stage2.status.success(),
        "stage2 list-strategies failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out_stage2.stdout),
        String::from_utf8_lossy(&out_stage2.stderr)
    );
    let stage2_stdout = String::from_utf8_lossy(&out_stage2.stdout);
    assert!(
        stage2_stdout.contains("stage2"),
        "expected discovered strategy in stage2 project output"
    );

    let out_stage1 = run_bt(
        &[
            "list-strategies",
            "--project",
            "stage1",
            "--workspace",
            ws.to_string_lossy().as_ref(),
            "--json",
        ],
        &root,
    );
    assert!(
        !out_stage1.status.success(),
        "stage1 list-strategies should fail when discovery compile/run fails\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out_stage1.stdout),
        String::from_utf8_lossy(&out_stage1.stderr)
    );
    let stage1_stderr = String::from_utf8_lossy(&out_stage1.stderr);
    assert!(
        stage1_stderr.contains("strategy discovery compile/run failed"),
        "expected discovery compile failure in stage1 project output"
    );

    let _ = fs::remove_dir_all(ws);
}
