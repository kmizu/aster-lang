#![forbid(unsafe_code)]

//! File-boundary validation and orchestration for the ASTER command line.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aster_diagnostics::{Diagnostic, DiagnosticCode, explain};
use aster_ir::{Program, lower};
use aster_runtime::{
    CapabilityGrants, EffectResolution, FixtureDriver, FixtureSet, Machine, MachineSnapshot,
    StartRequest, Step, Trace, record_run_evidenced, replay_run,
};
use aster_semantics::check_source;
use aster_syntax::{SourceFile, format_source, parse};
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// ASTER 0.1 compiler and deterministic fixture runtime.
#[derive(Debug, Parser)]
#[command(name = "aster", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Check {
        source: PathBuf,
        #[arg(long, value_enum, default_value_t)]
        diagnostic_format: DiagnosticFormat,
    },
    Fmt {
        source: PathBuf,
        #[arg(long, conflicts_with = "write")]
        check: bool,
        #[arg(long, conflicts_with = "check")]
        write: bool,
    },
    Ast {
        source: PathBuf,
        #[arg(long, required = true)]
        json: bool,
    },
    Run {
        source: PathBuf,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        event: String,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        capabilities: PathBuf,
        #[arg(long)]
        fixtures: PathBuf,
        #[arg(long)]
        trace: PathBuf,
        #[arg(long)]
        snapshot_dir: PathBuf,
        #[arg(long)]
        output_state: PathBuf,
        #[arg(long, value_enum, default_value_t)]
        diagnostic_format: DiagnosticFormat,
    },
    Replay {
        source: PathBuf,
        #[arg(long)]
        trace: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        capabilities: PathBuf,
        #[arg(long)]
        output_state: PathBuf,
        #[arg(long, value_enum, default_value_t)]
        diagnostic_format: DiagnosticFormat,
    },
    Resume {
        source: PathBuf,
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        resolution: PathBuf,
        #[arg(long)]
        trace: PathBuf,
        #[arg(long)]
        snapshot_dir: PathBuf,
        #[arg(long)]
        output_state: PathBuf,
    },
    Explain {
        diagnostic_code: String,
    },
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum DiagnosticFormat {
    #[default]
    Human,
    Json,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventInput {
    schema_version: u32,
    event_id: String,
    event_time: String,
    agent_arguments: BTreeMap<String, Value>,
    payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialState {
    schema_version: u32,
    state: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize)]
struct OutputState<'a> {
    schema_version: u32,
    state: &'a BTreeMap<String, Value>,
}

/// Parses arguments, performs one command, and maps failures to stable exit classes.
#[must_use]
pub fn main_entry() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn execute(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Check {
            source,
            diagnostic_format,
        } => {
            let _ = compile(&source, diagnostic_format)?;
            Ok(())
        }
        Command::Fmt {
            source,
            check,
            write,
        } => format_command(&source, check, write),
        Command::Ast { source, json: _ } => {
            let text = read_utf8(&source)?;
            let source_file = SourceFile::new(source.display().to_string(), text);
            let module =
                parse(&source_file).map_err(|values| source_error(&values, &source_file))?;
            println!("{}", serde_json::to_string(&module)?);
            Ok(())
        }
        Command::Run {
            source,
            agent,
            event,
            input,
            state,
            capabilities,
            fixtures,
            trace,
            snapshot_dir,
            output_state,
            diagnostic_format,
        } => run_command(RunFiles {
            source,
            agent,
            event,
            input,
            state,
            capabilities,
            fixtures,
            trace,
            snapshot_dir,
            output_state,
            diagnostic_format,
        }),
        Command::Replay {
            source,
            trace,
            input,
            state,
            capabilities,
            output_state,
            diagnostic_format,
        } => replay_command(
            &source,
            &trace,
            &input,
            &state,
            &capabilities,
            &output_state,
            diagnostic_format,
        ),
        Command::Resume {
            source,
            snapshot,
            resolution,
            trace,
            snapshot_dir,
            output_state,
        } => resume_command(
            &source,
            &snapshot,
            &resolution,
            &trace,
            &snapshot_dir,
            &output_state,
        ),
        Command::Explain { diagnostic_code } => explain_command(&diagnostic_code),
    }
}

struct RunFiles {
    source: PathBuf,
    agent: String,
    event: String,
    input: PathBuf,
    state: PathBuf,
    capabilities: PathBuf,
    fixtures: PathBuf,
    trace: PathBuf,
    snapshot_dir: PathBuf,
    output_state: PathBuf,
    diagnostic_format: DiagnosticFormat,
}

fn run_command(files: RunFiles) -> Result<(), CliError> {
    let program = compile(&files.source, files.diagnostic_format)?;
    let input: EventInput = read_json(&files.input)?;
    let state: InitialState = read_json(&files.state)?;
    let capabilities: CapabilityGrants = read_json(&files.capabilities)?;
    let fixtures: FixtureSet = read_json(&files.fixtures)?;
    validate_schema(input.schema_version, "event input")?;
    validate_schema(state.schema_version, "initial state")?;
    let start = start_request(files.agent, files.event, input, state, capabilities);
    let mut driver = FixtureDriver::new(fixtures).map_err(CliError::runtime)?;
    let recorded = match record_run_evidenced(program, start, &mut driver) {
        Ok(value) => value,
        Err(failure) => {
            write_snapshots(&files.snapshot_dir, &failure.snapshots)?;
            if !failure.trace.entries.is_empty() {
                atomic_write(&files.trace, failure.trace.to_json_lines()?.as_bytes())?;
            }
            return Err(CliError::runtime(failure.error));
        }
    };
    write_snapshots(&files.snapshot_dir, &recorded.snapshots)?;
    atomic_write(&files.trace, recorded.trace.to_json_lines()?.as_bytes())?;
    write_state(&files.output_state, &recorded.outcome.state)?;
    Ok(())
}

fn write_snapshots(directory: &Path, snapshots: &[MachineSnapshot]) -> Result<(), CliError> {
    fs::create_dir_all(directory)?;
    for (index, snapshot) in snapshots.iter().enumerate() {
        atomic_write(
            &directory.join(format!("snapshot-{index:04}.json")),
            snapshot.to_json()?.as_bytes(),
        )?;
    }
    Ok(())
}

fn replay_command(
    source: &Path,
    trace: &Path,
    input: &Path,
    state: &Path,
    capabilities: &Path,
    output_state: &Path,
    format: DiagnosticFormat,
) -> Result<(), CliError> {
    let program = compile(source, format)?;
    let trace = Trace::from_json_lines(&read_utf8(trace)?).map_err(CliError::replay)?;
    let input: EventInput = read_json(input)?;
    let state: InitialState = read_json(state)?;
    let capabilities: CapabilityGrants = read_json(capabilities)?;
    validate_schema(input.schema_version, "event input")?;
    validate_schema(state.schema_version, "initial state")?;
    let header = trace
        .entries
        .first()
        .ok_or_else(|| CliError::replay("trace has no run header"))?;
    let agent = json_string(&header.payload, "agent")?;
    let event = json_string(&header.payload, "event")?;
    let start = start_request(agent, event, input, state, capabilities);
    let outcome = replay_run(program, start, &trace).map_err(CliError::replay)?;
    write_state(output_state, &outcome.state)
}

fn resume_command(
    source: &Path,
    snapshot: &Path,
    resolution: &Path,
    trace_path: &Path,
    snapshot_dir: &Path,
    output_state: &Path,
) -> Result<(), CliError> {
    let program = compile(source, DiagnosticFormat::Human)?;
    let snapshot = MachineSnapshot::from_json(&read_utf8(snapshot)?)?;
    let resolution: EffectResolution = read_json(resolution)?;
    let mut trace = Trace::from_json_lines(&read_utf8(trace_path)?).map_err(CliError::replay)?;
    let mut machine = Machine::restore(program, snapshot)?;
    machine.supply(&resolution)?;
    trace.append("effect_resolved", serde_json::to_value(&resolution)?)?;
    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(request) => {
                fs::create_dir_all(snapshot_dir)?;
                let next = machine.snapshot()?;
                atomic_write(
                    &snapshot_dir.join("resume-next.json"),
                    next.to_json()?.as_bytes(),
                )?;
                trace.append("effect_requested", serde_json::to_value(request)?)?;
                atomic_write(trace_path, trace.to_json_lines()?.as_bytes())?;
                return Ok(());
            }
            Step::Completed(outcome) => {
                trace.append("state_committed", serde_json::to_value(&outcome.state)?)?;
                trace.append("run_completed", serde_json::to_value(&outcome)?)?;
                atomic_write(trace_path, trace.to_json_lines()?.as_bytes())?;
                return write_state(output_state, &outcome.state);
            }
            Step::Failed(error) => return Err(CliError::runtime(error)),
        }
    }
}

fn start_request(
    agent: String,
    event: String,
    input: EventInput,
    state: InitialState,
    capabilities: CapabilityGrants,
) -> StartRequest {
    StartRequest {
        agent,
        event,
        event_id: input.event_id,
        event_time: input.event_time,
        agent_arguments: input.agent_arguments,
        payload: input.payload,
        state: state.state,
        capabilities,
    }
}

fn format_command(path: &Path, check: bool, write: bool) -> Result<(), CliError> {
    let original = read_utf8(path)?;
    let source = SourceFile::new(path.display().to_string(), original.clone());
    let formatted = format_source(&source).map_err(|values| source_error(&values, &source))?;
    if check && original != formatted {
        return Err(CliError::Source(
            "source is not canonically formatted".to_owned(),
        ));
    }
    if write {
        atomic_write(path, formatted.as_bytes())?;
    } else if !check {
        print!("{formatted}");
    }
    Ok(())
}

fn compile(path: &Path, format: DiagnosticFormat) -> Result<Program, CliError> {
    let text = read_utf8(path)?;
    let source = SourceFile::new(path.display().to_string(), text);
    let checked = check_source(&source)
        .map_err(|values| source_error_with_format(&values, &source, format))?;
    lower(&checked).map_err(CliError::internal)
}

fn explain_command(code: &str) -> Result<(), CliError> {
    let code = DiagnosticCode::new(code.to_owned())
        .map_err(|_| CliError::Source("invalid diagnostic code".to_owned()))?;
    let value = explain(code.as_str())
        .ok_or_else(|| CliError::Source("unknown diagnostic code".to_owned()))?;
    println!(
        "{}\nmeaning: {}\ncause: {}\nremediation: {}",
        value.code.as_str(),
        value.meaning,
        value.cause,
        value.remediation
    );
    Ok(())
}

fn source_error(values: &[Diagnostic], source: &SourceFile) -> CliError {
    source_error_with_format(values, source, DiagnosticFormat::Human)
}

fn source_error_with_format(
    values: &[Diagnostic],
    source: &SourceFile,
    format: DiagnosticFormat,
) -> CliError {
    let rendered = match format {
        DiagnosticFormat::Human => values
            .iter()
            .map(|value| value.render_human(source.text()))
            .collect::<String>(),
        DiagnosticFormat::Json => match serde_json::to_string(values) {
            Ok(value) => value,
            Err(error) => format!("{{\"code\":\"ASTER-INTERNAL-9901\",\"message\":{error:?}}}"),
        },
    };
    CliError::Source(rendered)
}

fn validate_schema(version: u32, identity: &str) -> Result<(), CliError> {
    if version == 1 {
        Ok(())
    } else {
        Err(CliError::runtime(format!("{identity} schema mismatch")))
    }
}

fn json_string(value: &Value, name: &str) -> Result<String, CliError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| CliError::replay(format!("run header lacks `{name}`")))
}

fn read_utf8(path: &Path) -> Result<String, CliError> {
    String::from_utf8(fs::read(path)?)
        .map_err(|_| CliError::Source(format!("{} is not valid UTF-8", path.display())))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, CliError> {
    serde_json::from_slice(&fs::read(path)?).map_err(CliError::Json)
}

fn write_state(path: &Path, state: &BTreeMap<String, Value>) -> Result<(), CliError> {
    let bytes = serde_json::to_vec(&OutputState {
        schema_version: 1,
        state,
    })?;
    let mut bytes = bytes;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let parent = match path.parent() {
        Some(value) => value,
        None => Path::new("."),
    };
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| CliError::Io(error.error))?;
    Ok(())
}

#[derive(Debug, Error)]
enum CliError {
    #[error("{0}")]
    Source(String),
    #[error("ASTER-RUNTIME-9001: runtime failure: {0}")]
    Runtime(String),
    #[error("ASTER-REPLAY-10001: replay failure: {0}")]
    Replay(String),
    #[error("ASTER-INTERNAL-9901: internal failure: {0}")]
    Internal(String),
    #[error("I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON failure: {0}")]
    Json(serde_json::Error),
}

impl CliError {
    fn runtime(error: impl std::fmt::Display) -> Self {
        Self::Runtime(error.to_string())
    }

    fn replay(error: impl std::fmt::Display) -> Self {
        Self::Replay(error.to_string())
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        Self::Internal(error.to_string())
    }

    const fn exit_code(&self) -> u8 {
        match self {
            Self::Source(_) => 1,
            Self::Runtime(_) | Self::Io(_) | Self::Json(_) => 2,
            Self::Replay(_) => 3,
            Self::Internal(_) => 4,
        }
    }
}

impl From<serde_json::Error> for CliError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<aster_runtime::MachineError> for CliError {
    fn from(error: aster_runtime::MachineError) -> Self {
        Self::runtime(error)
    }
}

impl From<aster_runtime::TraceError> for CliError {
    fn from(error: aster_runtime::TraceError) -> Self {
        Self::runtime(error)
    }
}
