use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

type DynError = Box<dyn std::error::Error + Send + Sync>;

macro_rules! impl_display_via_debug {
    ($($t:ty),+ $(,)?) => {
        $(
            impl std::fmt::Display for $t {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{:?}", self)
                }
            }
        )+
    };
}

#[derive(Parser, Debug)]
#[command(name = "bt", about = "Backtesting workspace CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Manage bt workspace roots")]
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommands,
    },
    #[command(about = "Create and manage projects inside a workspace")]
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
    #[command(about = "Run a backtest for a project")]
    Run(RunArgs),
    #[command(about = "Run parameter sweep for a project")]
    Sweep(SweepArgs),
    #[command(about = "List discoverable strategies for a project")]
    ListStrategies(ListStrategiesArgs),
    #[command(about = "Clean generated files and cache artifacts")]
    Clean(CleanArgs),
}

#[derive(Subcommand, Debug)]
enum WorkspaceCommands {
    #[command(about = "Initialize a workspace (.bt/workspace.toml, projects/, cache)")]
    Init(WorkspaceInitArgs),
}

#[derive(Subcommand, Debug)]
enum ProjectCommands {
    #[command(about = "Scaffold a new project with strategy crate and bt.toml")]
    Init(ProjectInitArgs),
}

#[derive(Args, Debug)]
struct WorkspaceInitArgs {
    #[arg(long, help = "Workspace root path (defaults to current directory)")]
    path: Option<PathBuf>,
    #[arg(long, default_value_t = false, help = "Overwrite existing workspace files")]
    force: bool,
}

#[derive(Args, Debug)]
struct ProjectInitArgs {
    #[arg(help = "Project name (used for folder and scaffold naming)")]
    name: String,
    #[arg(long, help = "Workspace root path")]
    workspace: Option<PathBuf>,
    #[arg(long, default_value_t = false, help = "Overwrite existing project folder")]
    force: bool,
}

#[derive(Args, Debug)]
#[command(after_help = "Configuration sourcing for bt run:\n  Required:\n    --project\n\n  Optional (can be sourced):\n    --strategy      <- BT_STRATEGY <- [run].default_strategy <- built-in default\n    --data          <- BT_DATA <- [run].data <- built-in default\n    --start-date    <- BT_START_DATE <- [run].start_date\n    --end-date      <- BT_END_DATE <- [run].end_date\n    --log-time-mode <- [run].log_time_mode <- simulation\n\n  Params merge order:\n    [run.params] then BT_PARAMS (comma-separated k=v) then --params (repeatable)\n\n  Overall precedence:\n    CLI > environment variables > bt.toml > defaults")]
struct RunArgs {
    #[arg(long, help = "Project name inside workspace")]
    project: String,
    #[arg(long, help = "Workspace root path")]
    workspace: Option<PathBuf>,
    #[arg(long, help = "Strategy id to run")]
    strategy: Option<String>,
    #[arg(long, help = "Input parquet data path")]
    data: Option<String>,
    #[arg(long, help = "Simulation start date (YYYY-MM-DD)")]
    start_date: Option<String>,
    #[arg(long, help = "Simulation end date (YYYY-MM-DD)")]
    end_date: Option<String>,
    #[arg(long, help = "Optional config file path")]
    config: Option<PathBuf>,
    #[arg(long, help = "Logger timestamp mode (simulation|wall)")]
    log_time_mode: Option<String>,
    #[arg(long = "params", help = "Strategy/runtime parameter override as key=value (repeatable)")]
    params: Vec<String>,
}

#[derive(Args, Debug)]
struct SweepArgs {
    #[arg(long, help = "Project name inside workspace")]
    project: String,
    #[arg(long, help = "Workspace root path")]
    workspace: Option<PathBuf>,
    #[arg(long, help = "Sweep config file path")]
    config: PathBuf,
}

#[derive(Args, Debug)]
struct ListStrategiesArgs {
    #[arg(long, help = "Project name inside workspace")]
    project: String,
    #[arg(long, help = "Workspace root path")]
    workspace: Option<PathBuf>,
    #[arg(long, default_value_t = false, help = "Emit JSON output")]
    json: bool,
}

#[derive(Args, Debug)]
struct CleanArgs {
    #[arg(long, help = "Project name (required with --generated-only)")]
    project: Option<String>,
    #[arg(long, help = "Workspace root path")]
    workspace: Option<PathBuf>,
    #[arg(long, default_value_t = false, help = "Remove only generated files under project")]
    generated_only: bool,
    #[arg(long, default_value_t = false, help = "Remove workspace cache artifacts")]
    all_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceManifest {
    projects_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProjectFile {
    project: ProjectSection,
    engine: Option<EngineSection>,
    run: Option<RunSection>,
    strategy_registry: Option<StrategyRegistrySection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProjectSection {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct EngineSection {
    path: Option<String>,
    bin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RunSection {
    default_strategy: Option<String>,
    data: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    log_time_mode: Option<String>,
    initial_capital: Option<i64>,
    params: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StrategyRegistrySection {
    stage1_fallback: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct StrategyMetadataRecord {
    id: String,
    #[serde(default)]
    parameters: Vec<ParameterSpecRecord>,
}

#[derive(Debug, Clone, Deserialize)]
struct ParameterSpecRecord {
    name: String,
    kind: String,
    required: bool,
    default_value: Option<String>,
}

impl_display_via_debug!(
    Cli,
    Commands,
    WorkspaceCommands,
    ProjectCommands,
    WorkspaceInitArgs,
    ProjectInitArgs,
    RunArgs,
    SweepArgs,
    ListStrategiesArgs,
    CleanArgs,
    WorkspaceManifest,
    ProjectFile,
    ProjectSection,
    EngineSection,
    RunSection,
    StrategyRegistrySection,
    StrategyMetadataRecord,
    ParameterSpecRecord,
);

fn main() {
    if let Err(err) = run() {
        eprintln!("bt: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), DynError> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Workspace { command } => match command {
            WorkspaceCommands::Init(args) => workspace_init(args),
        },
        Commands::Project { command } => match command {
            ProjectCommands::Init(args) => project_init(args),
        },
        Commands::Run(args) => run_backtest(args),
        Commands::Sweep(args) => run_sweep(args),
        Commands::ListStrategies(args) => list_strategies(args),
        Commands::Clean(args) => clean(args),
    }
}

fn workspace_init(args: WorkspaceInitArgs) -> Result<(), DynError> {
    let root = resolve_root(args.path)?;
    let bt_dir = root.join(".bt");
    let workspace_file = bt_dir.join("workspace.toml");
    let projects_dir = root.join("projects");
    let cache_dir = bt_dir.join("cache");

    if workspace_file.exists() && !args.force {
        return Err(format!(
            "workspace already initialized at {} (use --force to overwrite)",
            workspace_file.display()
        )
        .into());
    }

    fs::create_dir_all(&projects_dir)?;
    fs::create_dir_all(&cache_dir)?;
    let manifest = WorkspaceManifest {
        projects_dir: Some("projects".to_string()),
    };
    fs::create_dir_all(&bt_dir)?;
    fs::write(&workspace_file, toml::to_string_pretty(&manifest)?)?;

    println!("initialized workspace: {}", root.display());
    Ok(())
}

fn project_init(args: ProjectInitArgs) -> Result<(), DynError> {
    let root = discover_workspace_root(args.workspace)?;
    let manifest = load_workspace_manifest(&root)?;
    let projects_dir = root.join(
        manifest
            .projects_dir
            .as_deref()
            .unwrap_or("projects"),
    );
    let project_root = projects_dir.join(&args.name);

    if project_root.exists() {
        if !args.force {
            return Err(format!(
                "project '{}' already exists at {} (use --force to overwrite)",
                args.name,
                project_root.display()
            )
            .into());
        }
        fs::remove_dir_all(&project_root)?;
    }

    let strategy_id = to_snake_identifier(&args.name);
    let crate_name = strategy_id.clone();
    let struct_base = to_pascal_identifier(&strategy_id);
    let strategy_struct = if struct_base.ends_with("Strategy") {
        struct_base.clone()
    } else {
        format!("{}Strategy", struct_base)
    };
    let strategy_display_name = to_display_name(&strategy_id);
    let registration_fn = format!("{}_registration", strategy_id);
    let strategy_file_name = format!("{}.rs", strategy_id);

    fs::create_dir_all(project_root.join("strategy/src"))?;
    fs::create_dir_all(project_root.join("generated"))?;

    let project_file = ProjectFile {
        project: ProjectSection {
            name: args.name.clone(),
        },
        engine: Some(EngineSection {
            path: Some(env!("CARGO_MANIFEST_DIR").to_string()),
            bin: Some("opt_bt".to_string()),
        }),
        run: Some(RunSection {
            default_strategy: Some(strategy_id.clone()),
            data: Some("./sample_data/niftyIndex2024.sample.parquet".to_string()),
            start_date: None,
            end_date: None,
            log_time_mode: Some("simulation".to_string()),
            initial_capital: Some(1_000_000),
            params: Some(BTreeMap::from([
                ("prob".to_string(), "0.5".to_string()),
                ("qty".to_string(), "1".to_string()),
                ("seed".to_string(), "42".to_string()),
            ])),
        }),
        strategy_registry: Some(StrategyRegistrySection {
            stage1_fallback: Some(true),
        }),
    };
    fs::write(
        project_root.join("bt.toml"),
        toml::to_string_pretty(&project_file)?,
    )?;

    fs::write(
        project_root.join("strategy/Cargo.toml"),
        format!(
            r#"[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2021"

[lib]
path = "src/{strategy_file_name}"

[dependencies]
opt_bt = {{ path = "{root}" }}
bt_strategy_sdk = {{ path = "{root}/crates/bt_strategy_sdk" }}
bt_strategy_macros = {{ path = "{root}/crates/bt_strategy_macros" }}
serde_json = "1"
log = "0.4"
"#,
            crate_name = crate_name,
            strategy_file_name = strategy_file_name,
            root = env!("CARGO_MANIFEST_DIR")
        ),
    )?;
    fs::write(
        project_root.join(format!("strategy/src/{}", strategy_file_name)),
        format!(
            r#"use bt_strategy_macros::bt_strategy;
use opt_bt::common::context::Context;
use opt_bt::common::event::{{FillEvent, MarketEvent, OrderEvent, SignalEvent}};
use opt_bt::common::types::{{OrderType, Side}};
use opt_bt::strategy::Strategy;

#[bt_strategy(
    id = "{strategy_id}",
    display_name = "{strategy_display_name}",
    description = "Sample scaffold strategy with simple bullish bar behavior"
)]
pub fn {registration_fn}() {{}}

pub struct {strategy_struct};

impl Strategy for {strategy_struct} {{
    fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {{
        if let Some(bar) = ctx.get_bar(event.instrument_id) {{
            if bar.close > bar.open {{
                let _ = ctx.place_order(event.instrument_id, Side::Buy, OrderType::Market, 1);
            }}
        }}
    }}

    fn on_signal(&mut self, _ctx: &mut Context, _event: &SignalEvent) {{}}
    fn on_order_event(&mut self, _ctx: &mut Context, _event: &OrderEvent) {{}}
    fn on_fill(&mut self, _ctx: &mut Context, _event: &FillEvent) {{}}
}}

pub fn create_strategy_by_id(
    strategy_id: &str,
    _params: &std::collections::BTreeMap<String, String>,
) -> Option<Box<dyn Strategy + Send>> {{
    match strategy_id {{
        "{strategy_id}" => Some(Box::new({strategy_struct})),
        _ => None,
    }}
}}
"#,
            strategy_id = strategy_id,
            strategy_display_name = strategy_display_name,
            registration_fn = registration_fn,
            strategy_struct = strategy_struct,
        ),
    )?;

    fs::create_dir_all(project_root.join("strategy/src/bin"))?;
    fs::write(
        project_root.join("strategy/src/bin/bt_list_strategies.rs"),
        format!(
            r#"use {crate_name} as _;

fn main() {{
    let metadata = bt_strategy_sdk::all_metadata();
    let json = serde_json::to_string_pretty(&metadata).expect("serialize strategy metadata");
    println!("{{}}", json);
}}
"#,
            crate_name = crate_name,
        ),
    )?;
    fs::write(
        project_root.join("strategy/src/bin/bt_run_project.rs"),
        format!(
            r#"use std::collections::BTreeMap;

use {crate_name}::create_strategy_by_id;
use opt_bt::common::logging;
use opt_bt::common::types::PRICE_SCALE;
use opt_bt::data::loader::DataLoader;
use opt_bt::engine::runner::Engine;
use opt_bt::reporting::json::generate_report;
use opt_bt::strategy::portfolio::PortfolioStrategy;

fn parse_params(values: &[String]) -> Result<BTreeMap<String, String>, String> {{
    let mut params = BTreeMap::new();
    for token in values {{
        let Some((key, value)) = token.split_once('=') else {{
            return Err(format!("invalid --params token '{{}}': expected key=value", token));
        }};
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {{
            return Err(format!("invalid --params token '{{}}': empty key", token));
        }}
        params.insert(key.to_string(), value.to_string());
    }}
    Ok(params)
}}

fn main() {{
    let mut strategy_id: Option<String> = None;
    let mut data: Option<String> = None;
    let mut start_date: Option<String> = None;
    let mut end_date: Option<String> = None;
    let mut initial_capital: i64 = 1_000_000;
    let mut log_level = "info".to_string();
    let mut log_time_mode = "simulation".to_string();
    let mut log_file: Option<String> = None;
    let mut report_path: Option<String> = None;
    let mut raw_params: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {{
        match arg.as_str() {{
            "--strategy-id" => strategy_id = args.next(),
            "--data" => data = args.next(),
            "--initial-capital" => {{
                if let Some(value) = args.next() {{
                    initial_capital = value.parse::<i64>().unwrap_or(1_000_000);
                }}
            }}
            "--log-level" => {{
                if let Some(value) = args.next() {{
                    log_level = value;
                }}
            }}
            "--log-time-mode" => {{
                if let Some(value) = args.next() {{
                    log_time_mode = value;
                }}
            }}
            "--log-file" => log_file = args.next(),
            "--report-path" => report_path = args.next(),
            "--params" => {{
                if let Some(value) = args.next() {{
                    raw_params.push(value);
                }}
            }}
            "--start-date" => start_date = args.next(),
            "--end-date" => end_date = args.next(),
            _ => {{}}
        }}
    }}

    let strategy_id = strategy_id.unwrap_or_else(|| "{strategy_id}".to_string());
    let data = data.unwrap_or_else(|| "./sample_data/niftyIndex2024.sample.parquet".to_string());

    let params = parse_params(&raw_params).unwrap_or_else(|err| panic!("Failed to parse params: {{}}", err));
    let _logger_guard = logging::init_with_time_mode_and_file(
        &log_level,
        Some(&log_time_mode),
        log_file.as_deref(),
    );
    log::info!(
        "Backtest config: strategy={{}} start_date={{}} end_date={{}} data={{}} initial_capital={{}} log_level={{}} log_time_mode={{}}",
        strategy_id,
        start_date.clone().unwrap_or_else(|| "<none>".to_string()),
        end_date.clone().unwrap_or_else(|| "<none>".to_string()),
        data,
        initial_capital,
        log_level,
        log_time_mode
    );
    let market_data = DataLoader::load_parquet(&data)
        .unwrap_or_else(|err| panic!("Failed to load parquet data: {{}}", err));

    let strategy = create_strategy_by_id(&strategy_id, &params)
        .unwrap_or_else(|| panic!("Unknown project strategy id: {{}}", strategy_id));

    let mut portfolio = PortfolioStrategy::new();
    portfolio.add_strategy("default", strategy);

    let mut engine = Engine::new(portfolio, market_data, initial_capital * PRICE_SCALE);
    engine.init();
    engine.run();

    let report = generate_report(&engine);
    let json = serde_json::to_string_pretty(&report).expect("serialize report");
    if let Some(path) = report_path {{
        std::fs::write(path, &json).expect("write report");
    }}
    println!("{{}}", json);
}}
"#,
            crate_name = crate_name,
            strategy_id = strategy_id,
        ),
    )?;

    sync_generated_files(&project_root)?;
    println!("initialized project: {}", project_root.display());
    Ok(())
}

fn run_backtest(args: RunArgs) -> Result<(), DynError> {
    let root = discover_workspace_root(args.workspace)?;
    let manifest = load_workspace_manifest(&root)?;
    let project_root = resolve_project_root(&root, &manifest, &args.project)?;
    sync_generated_files(&project_root)?;

    let project_file = load_project_file(&project_root)?;
    let allow_stage1_fallback = stage1_fallback_enabled(&project_file);
    let engine_path = project_file
        .engine
        .as_ref()
        .and_then(|e| e.path.as_deref())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let engine_bin = project_file
        .engine
        .as_ref()
        .and_then(|e| e.bin.as_deref())
        .unwrap_or("opt_bt");

    let run_cfg = project_file.run.unwrap_or_default();
    let env_strategy = std::env::var("BT_STRATEGY").ok();
    let env_data = std::env::var("BT_DATA").ok();
    let env_start_date = std::env::var("BT_START_DATE").ok();
    let env_end_date = std::env::var("BT_END_DATE").ok();
    let env_params = std::env::var("BT_PARAMS").ok();
    let default_strategy = run_cfg.default_strategy.clone();
    let log_time_mode = args
        .log_time_mode
        .or(run_cfg.log_time_mode.clone())
        .unwrap_or_else(|| "simulation".to_string());

    let strategy = args
        .strategy
        .or(env_strategy)
        .or(default_strategy)
        .unwrap_or_else(|| "random".to_string());

    let output_dir = create_backtest_output_dir(&project_root, &strategy)?;
    let log_path = output_dir.join("engine.log");
    let report_path = output_dir.join("report.json");

    let known = resolve_runtime_strategy_ids(&project_root, allow_stage1_fallback)?;
    if !known.iter().any(|item| item == &strategy) {
        return Err(format!(
            "unknown strategy id '{}' (known: {})",
            strategy,
            known.join(", ")
        )
        .into());
    }

    let metadata = discover_strategy_metadata(&project_root)?;
    let merged_params = merge_runtime_params(run_cfg.params.clone(), env_params.clone(), &args.params)?;
    validate_strategy_params(&strategy, &merged_params, &metadata)?;

    let is_builtin_strategy = known_stage1_strategies()
        .iter()
        .any(|known_id| known_id == &strategy);

    let data = args
        .data
        .or(env_data)
        .or(run_cfg.data)
        .unwrap_or_else(|| "./sample_data/niftyIndex2024.sample.parquet".to_string());

    if !Path::new(&data).exists() {
        return Err(format!("configured data path does not exist: {data}").into());
    }

    let start_date = args.start_date.or(env_start_date).or(run_cfg.start_date);
    let end_date = args.end_date.or(env_end_date).or(run_cfg.end_date);

    let status = if is_builtin_strategy {
        let engine_strategy = if strategy == "sample_strategy" {
            "random".to_string()
        } else {
            strategy.clone()
        };

        let profile = engine_profile();
        let mut cmd = Command::new("cargo");
        cmd.current_dir(engine_path)
            .args(cargo_run_prefix(engine_bin, &profile))
            .arg("run")
            .args(["--strategy", &engine_strategy])
            .args(["--data-dir", &data])
            .args(["--log-file", log_path.to_string_lossy().as_ref()])
            .args(["--report-path", report_path.to_string_lossy().as_ref()]);

        if let Some(config) = args.config {
            cmd.args(["--config-file", config.to_string_lossy().as_ref()]);
        }
        if let Some(ref resolved_start_date) = start_date {
            cmd.args(["--start-date", resolved_start_date]);
        }
        if let Some(ref resolved_end_date) = end_date {
            cmd.args(["--end-date", resolved_end_date]);
        }
        cmd.args(["--log-time-mode", &log_time_mode]);

        for (key, value) in &merged_params {
            cmd.args(["--params", &format!("{key}={value}")]);
        }

        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?
    } else {
        run_project_strategy(
            &project_root,
            &strategy,
            &data,
            run_cfg.initial_capital.unwrap_or(1_000_000),
            &log_time_mode,
            &merged_params,
            start_date,
            end_date,
            &log_path,
            &report_path,
        )?
    };

    if !status.success() {
        return Err(format!("bt run failed with status: {status}").into());
    }
    eprintln!(
        "bt: artifacts written to {}",
        output_dir.to_string_lossy()
    );
    Ok(())
}

fn run_sweep(args: SweepArgs) -> Result<(), DynError> {
    let root = discover_workspace_root(args.workspace)?;
    let manifest = load_workspace_manifest(&root)?;
    let project_root = resolve_project_root(&root, &manifest, &args.project)?;
    sync_generated_files(&project_root)?;

    let project_file = load_project_file(&project_root)?;
    let allow_stage1_fallback = stage1_fallback_enabled(&project_file);
    let engine_path = project_file
        .engine
        .as_ref()
        .and_then(|e| e.path.as_deref())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let engine_bin = project_file
        .engine
        .as_ref()
        .and_then(|e| e.bin.as_deref())
        .unwrap_or("opt_bt");

    let run_cfg = project_file.run.unwrap_or_default();
    let strategy = run_cfg
        .default_strategy
        .clone()
        .unwrap_or_else(|| "random".to_string());
    let known = resolve_runtime_strategy_ids(&project_root, allow_stage1_fallback)?;
    if !known.iter().any(|item| item == &strategy) {
        return Err(format!(
            "unknown strategy id '{}' (known: {})",
            strategy,
            known.join(", ")
        )
        .into());
    }
    let metadata = discover_strategy_metadata(&project_root)?;
    let merged_params = merge_runtime_params(run_cfg.params, None, &[])?;
    validate_strategy_params(&strategy, &merged_params, &metadata)?;

    let profile = engine_profile();
    let mut cmd = Command::new("cargo");
    let status = cmd
        .current_dir(engine_path)
        .args(cargo_run_prefix(engine_bin, &profile))
        .arg("sweep")
        .arg("--config")
        .arg(args.config)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        return Err(format!("bt sweep failed with status: {status}").into());
    }
    Ok(())
}

fn clean(args: CleanArgs) -> Result<(), DynError> {
    let root = discover_workspace_root(args.workspace)?;
    let manifest = load_workspace_manifest(&root)?;
    let has_project = args.project.is_some();

    if let Some(project) = args.project.as_deref() {
        let project_root = resolve_project_root(&root, &manifest, project)?;
        let generated = project_root.join("generated");
        if generated.exists() {
            fs::remove_dir_all(&generated)?;
            println!("removed generated files: {}", generated.display());
        }
    }

    if args.all_cache {
        let cache_dir = root.join(".bt/cache");
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir)?;
            println!("removed cache: {}", cache_dir.display());
        }
    }

    if args.generated_only && !args.all_cache && !has_project {
        return Err("--generated-only requires --project <name>".into());
    }

    Ok(())
}

fn list_strategies(args: ListStrategiesArgs) -> Result<(), DynError> {
    let root = discover_workspace_root(args.workspace)?;
    let manifest = load_workspace_manifest(&root)?;
    let project_root = resolve_project_root(&root, &manifest, &args.project)?;
    let project_file = load_project_file(&project_root)?;
    let allow_stage1_fallback = stage1_fallback_enabled(&project_file);
    if let Some(text) = discover_strategy_metadata_json(&project_root)? {
        if args.json {
            println!("{}", text);
        } else {
            print_human_strategy_list(&text)?;
        }
        return Ok(());
    }

    if !allow_stage1_fallback {
        return Err(
            "strategy discovery failed and stage-1 fallback is disabled in project config"
                .into(),
        );
    }

    let fallback = known_stage1_strategies();
    if args.json {
        let json = serde_json::to_string_pretty(&fallback)?;
        println!("{}", json);
    } else {
        println!("Stage-1 fallback strategy list:");
        for item in fallback {
            println!("- {}", item);
        }
    }
    Ok(())
}

fn resolve_root(path: Option<PathBuf>) -> Result<PathBuf, DynError> {
    let root = match path {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    Ok(root)
}

fn discover_workspace_root(path: Option<PathBuf>) -> Result<PathBuf, DynError> {
    let root = resolve_root(path)?;
    let manifest = root.join(".bt/workspace.toml");
    if !manifest.exists() {
        return Err(format!(
            "workspace not initialized at {} (run: bt workspace init)",
            root.display()
        )
        .into());
    }
    Ok(root)
}

fn load_workspace_manifest(root: &Path) -> Result<WorkspaceManifest, DynError> {
    let path = root.join(".bt/workspace.toml");
    let content = fs::read_to_string(&path)?;
    let manifest: WorkspaceManifest = toml::from_str(&content)?;
    Ok(manifest)
}

fn resolve_project_root(
    root: &Path,
    manifest: &WorkspaceManifest,
    project: &str,
) -> Result<PathBuf, DynError> {
    let projects_dir = root.join(
        manifest
            .projects_dir
            .as_deref()
            .unwrap_or("projects"),
    );
    let project_root = projects_dir.join(project);
    if !project_root.exists() {
        return Err(format!("project '{}' not found at {}", project, project_root.display()).into());
    }
    Ok(project_root)
}

fn load_project_file(project_root: &Path) -> Result<ProjectFile, DynError> {
    let path = project_root.join("bt.toml");
    if !path.exists() {
        return Err(format!("project config not found: {}", path.display()).into());
    }
    let content = fs::read_to_string(path)?;
    let cfg: ProjectFile = toml::from_str(&content)?;
    Ok(cfg)
}

fn sync_generated_files(project_root: &Path) -> Result<(), DynError> {
    let generated = project_root.join("generated");
    fs::create_dir_all(&generated)?;
    let registry = generated.join("strategy_registry.rs");
    let content = r#"// stage-1 generated registry placeholder
// In stage 1, bt maintains deterministic static registration glue.
pub const STAGE: &str = "stage1-static";
pub const STRATEGIES: &[&str] = &[
    "sample_strategy",
    "random",
    "atm_straddle",
    "atm_straddle_sell",
    "nifty_nearest_expiry_straddle",
    "sma_nifty50",
];
"#;
    fs::write(registry, content)?;
    Ok(())
}

fn run_project_strategy(
    project_root: &Path,
    strategy_id: &str,
    data: &str,
    initial_capital: i64,
    log_time_mode: &str,
    params: &BTreeMap<String, String>,
    start_date: Option<String>,
    end_date: Option<String>,
    log_path: &Path,
    report_path: &Path,
) -> Result<std::process::ExitStatus, DynError> {
    let strategy_manifest = project_root.join("strategy/Cargo.toml");
    if !strategy_manifest.exists() {
        return Err(format!(
            "strategy manifest not found for project strategy execution: {}",
            strategy_manifest.display()
        )
        .into());
    }

    let data_path = std::fs::canonicalize(data)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| data.to_string());

    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--manifest-path")
        .arg(&strategy_manifest)
        .arg("--bin")
        .arg("bt_run_project")
        .arg("--")
        .args(["--strategy-id", strategy_id])
        .args(["--data", &data_path])
        .args(["--initial-capital", &initial_capital.to_string()])
        .args(["--log-time-mode", log_time_mode])
        .args(["--log-file", log_path.to_string_lossy().as_ref()])
        .args(["--report-path", report_path.to_string_lossy().as_ref()])
        .args(["--log-level", "info"]);

    if let Some(ref value) = start_date {
        cmd.args(["--start-date", value]);
    }
    if let Some(ref value) = end_date {
        cmd.args(["--end-date", value]);
    }

    for (key, value) in params {
        cmd.args(["--params", &format!("{key}={value}")]);
    }

    let status = cmd
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    Ok(status)
}

fn create_backtest_output_dir(project_root: &Path, strategy: &str) -> Result<PathBuf, DynError> {
    let backtests_root = project_root.join("backtests");
    fs::create_dir_all(&backtests_root)?;

    let strategy_id = to_snake_identifier(strategy);
    let timestamp = chrono::Local::now().format("%Y_%m_%d_%H_%M_%S").to_string();
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|dur| dur.subsec_nanos())
        .unwrap_or(0);

    let dir_name = format!("{}_{}_{}", strategy_id, timestamp, run_id);
    let output_dir = backtests_root.join(dir_name);
    fs::create_dir_all(&output_dir)?;
    Ok(output_dir)
}

fn to_snake_identifier(input: &str) -> String {
    let mut result = String::new();
    let mut last_was_underscore = false;

    for character in input.chars() {
        if character.is_ascii_alphanumeric() {
            result.push(character.to_ascii_lowercase());
            last_was_underscore = false;
        } else if !last_was_underscore {
            result.push('_');
            last_was_underscore = true;
        }
    }

    while result.starts_with('_') {
        result.remove(0);
    }
    while result.ends_with('_') {
        result.pop();
    }

    if result.is_empty() {
        result = "strategy".to_string();
    }
    if result
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
    {
        result.insert_str(0, "strategy_");
    }

    result
}

fn to_pascal_identifier(snake: &str) -> String {
    let mut output = String::new();
    for part in snake.split('_').filter(|part| !part.is_empty()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            output.push(first.to_ascii_uppercase());
            output.extend(chars);
        }
    }

    if output.is_empty() {
        "Strategy".to_string()
    } else if output
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("Strategy{}", output)
    } else {
        output
    }
}

fn to_display_name(snake: &str) -> String {
    let words: Vec<String> = snake
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut title = String::new();
                    title.push(first.to_ascii_uppercase());
                    title.extend(chars.map(|c| c.to_ascii_lowercase()));
                    title
                }
                None => String::new(),
            }
        })
        .collect();

    if words.is_empty() {
        "Sample Strategy".to_string()
    } else {
        words.join(" ")
    }
}

fn known_stage1_strategies() -> Vec<String> {
    vec![
        "sample_strategy".to_string(),
        "random".to_string(),
        "atm_straddle".to_string(),
        "atm_straddle_sell".to_string(),
        "nifty_nearest_expiry_straddle".to_string(),
        "sma_nifty50".to_string(),
    ]
}

fn resolve_runtime_strategy_ids(project_root: &Path, allow_stage1_fallback: bool) -> Result<Vec<String>, DynError> {
    let discovered = discover_strategy_metadata(project_root)?;
    if !discovered.is_empty() {
        let mut ids: Vec<String> = discovered.into_iter().map(|item| item.id).collect();
        ids.sort();
        ids.dedup();
        if !ids.is_empty() {
            return Ok(ids);
        }
    }

    if !allow_stage1_fallback {
        return Err(
            "strategy discovery failed and stage-1 fallback is disabled in project config"
                .into(),
        );
    }

    Ok(known_stage1_strategies())
}

fn stage1_fallback_enabled(project_file: &ProjectFile) -> bool {
    project_file
        .strategy_registry
        .as_ref()
        .and_then(|cfg| cfg.stage1_fallback)
        .unwrap_or(true)
}

fn discover_strategy_metadata(project_root: &Path) -> Result<Vec<StrategyMetadataRecord>, DynError> {
    if let Some(json) = discover_strategy_metadata_json(project_root)? {
        let discovered: Vec<StrategyMetadataRecord> = serde_json::from_str(&json)?;
        return Ok(discovered);
    }
    Ok(Vec::new())
}

fn merge_runtime_params(
    config_params: Option<BTreeMap<String, String>>,
    env_params: Option<String>,
    cli_params: &[String],
) -> Result<BTreeMap<String, String>, DynError> {
    let mut merged = BTreeMap::new();

    if let Some(params) = config_params {
        for (key, value) in params {
            merged.insert(key, value);
        }
    }

    if let Some(raw) = env_params {
        for token in raw.split(',') {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (key, value) = parse_param_token(trimmed)?;
            merged.insert(key, value);
        }
    }

    for token in cli_params {
        let (key, value) = parse_param_token(token)?;
        merged.insert(key, value);
    }

    Ok(merged)
}

fn parse_param_token(token: &str) -> Result<(String, String), DynError> {
    let Some((key, value)) = token.split_once('=') else {
        return Err(format!("invalid --params token '{}': expected key=value", token).into());
    };

    let key = key.trim();
    let value = value.trim();
    if key.is_empty() {
        return Err(format!("invalid --params token '{}': empty key", token).into());
    }

    Ok((key.to_string(), value.to_string()))
}

fn validate_strategy_params(
    strategy_id: &str,
    params: &BTreeMap<String, String>,
    metadata: &[StrategyMetadataRecord],
) -> Result<(), DynError> {
    let Some(strategy_meta) = metadata.iter().find(|item| item.id == strategy_id) else {
        return Ok(());
    };

    if strategy_meta.parameters.is_empty() {
        return Ok(());
    }

    let mut specs = BTreeMap::new();
    for spec in &strategy_meta.parameters {
        specs.insert(spec.name.as_str(), spec);
    }

    let unknown: Vec<&str> = params
        .keys()
        .filter(|key| !specs.contains_key(key.as_str()))
        .map(|key| key.as_str())
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "unknown parameter(s) for strategy '{}': {}",
            strategy_id,
            unknown.join(", ")
        )
        .into());
    }

    for spec in &strategy_meta.parameters {
        if spec.required && spec.default_value.is_none() && !params.contains_key(spec.name.as_str()) {
            return Err(
                format!("missing required parameter '{}' for strategy '{}'", spec.name, strategy_id)
                    .into(),
            );
        }
    }

    for (key, value) in params {
        let spec = specs
            .get(key.as_str())
            .ok_or_else(|| format!("missing spec for parameter '{}'", key))?;
        validate_parameter_kind(strategy_id, key, value, &spec.kind)?;
    }

    Ok(())
}

fn validate_parameter_kind(
    strategy_id: &str,
    key: &str,
    value: &str,
    kind: &str,
) -> Result<(), DynError> {
    match kind {
        "string" => Ok(()),
        "int" | "integer" | "i64" => {
            value.parse::<i64>().map_err(|_| {
                format!(
                    "invalid value for parameter '{}' (strategy '{}'): expected integer, got '{}'",
                    key, strategy_id, value
                )
            })?;
            Ok(())
        }
        "float" | "number" | "f64" => {
            value.parse::<f64>().map_err(|_| {
                format!(
                    "invalid value for parameter '{}' (strategy '{}'): expected float, got '{}'",
                    key, strategy_id, value
                )
            })?;
            Ok(())
        }
        "bool" | "boolean" => {
            let lower = value.to_ascii_lowercase();
            if matches!(lower.as_str(), "true" | "false" | "1" | "0") {
                Ok(())
            } else {
                Err(format!(
                    "invalid value for parameter '{}' (strategy '{}'): expected bool, got '{}'",
                    key, strategy_id, value
                )
                .into())
            }
        }
        _ => Err(format!(
            "unknown parameter kind '{}' for '{}' in strategy '{}'",
            kind, key, strategy_id
        )
        .into()),
    }
}

fn discover_strategy_metadata_json(project_root: &Path) -> Result<Option<String>, DynError> {
    let strategy_manifest = project_root.join("strategy/Cargo.toml");
    if !strategy_manifest.exists() {
        return Ok(None);
    }

    let output = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg(strategy_manifest.as_os_str())
        .arg("--bin")
        .arg("bt_list_strategies")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let trimmed = stderr.trim();
        if trimmed.is_empty() {
            return Err("strategy discovery failed (bt_list_strategies exited non-zero)".into());
        }

        return Err(format!(
            "strategy discovery compile/run failed:\n{}",
            trimmed
        )
        .into());
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        return Ok(None);
    }

    Ok(Some(text))
}

fn print_human_strategy_list(json: &str) -> Result<(), DynError> {
    let parsed: serde_json::Value = serde_json::from_str(json)?;
    let arr = parsed.as_array().ok_or("strategy metadata output must be array")?;

    println!("Discovered strategies:");
    for item in arr {
        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let name = item
            .get("display_name")
            .and_then(|v| v.as_str())
            .unwrap_or(id);
        let description = item
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        println!("- {} ({})", id, name);
        if !description.is_empty() {
            println!("  {}", description);
        }
    }
    Ok(())
}

fn engine_profile() -> String {
    std::env::var("BT_ENGINE_PROFILE").unwrap_or_else(|_| "release".to_string())
}

fn cargo_run_prefix<'a>(engine_bin: &'a str, profile: &str) -> Vec<&'a str> {
    let mut args = vec!["run"];
    if profile == "release" {
        args.push("--release");
    }
    args.push("--bin");
    args.push(engine_bin);
    args.push("--");
    args
}
