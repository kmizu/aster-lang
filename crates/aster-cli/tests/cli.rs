use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn aster() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aster"))
}

fn example(name: &str) -> PathBuf {
    root().join("examples/meeting-scheduler").join(name)
}

fn run_args(directory: &Path) -> Vec<String> {
    vec![
        "run".to_owned(),
        example("main.aster").display().to_string(),
        "--agent".to_owned(),
        "Scheduler".to_owned(),
        "--event".to_owned(),
        "message".to_owned(),
        "--input".to_owned(),
        example("event.json").display().to_string(),
        "--state".to_owned(),
        example("initial-state.json").display().to_string(),
        "--capabilities".to_owned(),
        example("capabilities.json").display().to_string(),
        "--fixtures".to_owned(),
        example("fixtures.json").display().to_string(),
        "--trace".to_owned(),
        directory.join("trace.jsonl").display().to_string(),
        "--snapshot-dir".to_owned(),
        directory.join("snapshots").display().to_string(),
        "--output-state".to_owned(),
        directory.join("record.json").display().to_string(),
    ]
}

#[test]
fn public_commands_check_format_ast_record_and_replay() {
    let directory = tempfile::tempdir().expect("temporary directory");
    assert!(
        aster()
            .args(["check", example("main.aster").to_str().expect("UTF-8 path")])
            .status()
            .expect("check executes")
            .success()
    );
    assert!(
        aster()
            .args([
                "fmt",
                example("main.aster").to_str().expect("UTF-8 path"),
                "--check",
            ])
            .status()
            .expect("format executes")
            .success()
    );
    let ast = aster()
        .args([
            "ast",
            example("main.aster").to_str().expect("UTF-8 path"),
            "--json",
        ])
        .output()
        .expect("AST executes");
    assert!(ast.status.success());
    let _: serde_json::Value = serde_json::from_slice(&ast.stdout).expect("AST is JSON");

    assert!(
        aster()
            .args(run_args(directory.path()))
            .status()
            .expect("record executes")
            .success()
    );
    let replay = directory.path().join("replay.json");
    assert!(
        aster()
            .args([
                "replay",
                example("main.aster").to_str().expect("UTF-8 path"),
                "--trace",
                directory
                    .path()
                    .join("trace.jsonl")
                    .to_str()
                    .expect("UTF-8 path"),
                "--input",
                example("event.json").to_str().expect("UTF-8 path"),
                "--state",
                example("initial-state.json").to_str().expect("UTF-8 path"),
                "--capabilities",
                example("capabilities.json").to_str().expect("UTF-8 path"),
                "--output-state",
                replay.to_str().expect("UTF-8 path"),
            ])
            .status()
            .expect("replay executes")
            .success()
    );
    assert_eq!(
        fs::read(directory.path().join("record.json")).expect("recorded state"),
        fs::read(replay).expect("replayed state")
    );
}

#[test]
fn exit_codes_distinguish_source_runtime_and_replay_failures() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let bad_source = root().join("tests/conformance/fail/candidate_used_without_validation.aster");
    let source_status = aster()
        .args(["check", bad_source.to_str().expect("UTF-8 path")])
        .status()
        .expect("source check executes");
    assert_eq!(source_status.code(), Some(1));

    let missing_capabilities = directory.path().join("missing-capabilities.json");
    fs::write(
        &missing_capabilities,
        "{\"schema_version\":1,\"grants\":[]}",
    )
    .expect("capability fixture writes");
    let mut runtime_args = run_args(directory.path());
    let capability_index = runtime_args
        .iter()
        .position(|value| value == "--capabilities")
        .expect("capabilities argument")
        + 1;
    runtime_args[capability_index] = missing_capabilities.display().to_string();
    let runtime_status = aster()
        .args(runtime_args)
        .status()
        .expect("runtime executes");
    assert_eq!(runtime_status.code(), Some(2));

    assert!(
        aster()
            .args(run_args(directory.path()))
            .status()
            .expect("record executes")
            .success()
    );
    let trace_path = directory.path().join("trace.jsonl");
    let mut trace = fs::read_to_string(&trace_path).expect("trace reads");
    trace = trace.replacen("Planning", "Tampered", 1);
    fs::write(&trace_path, trace).expect("trace tampers");
    let replay_status = aster()
        .args([
            "replay",
            example("main.aster").to_str().expect("UTF-8 path"),
            "--trace",
            trace_path.to_str().expect("UTF-8 path"),
            "--input",
            example("event.json").to_str().expect("UTF-8 path"),
            "--state",
            example("initial-state.json").to_str().expect("UTF-8 path"),
            "--capabilities",
            example("capabilities.json").to_str().expect("UTF-8 path"),
            "--output-state",
            directory
                .path()
                .join("replay.json")
                .to_str()
                .expect("UTF-8 path"),
        ])
        .status()
        .expect("replay executes");
    assert_eq!(replay_status.code(), Some(3));
}
