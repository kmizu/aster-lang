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

fn assert_resume(directory: &Path) {
    let record_trace = directory.join("trace.jsonl");
    let resume_trace = directory.join("resume.trace.jsonl");
    fs::copy(&record_trace, &resume_trace).expect("resume trace copy");
    let resolution_path = directory.join("resolution.json");
    let resolutions: Vec<_> = fs::read_to_string(&record_trace)
        .expect("trace reads")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("trace line is JSON"))
        .filter(|entry| entry["kind"] == "effect_resolved")
        .map(|entry| entry["payload"].clone())
        .collect();
    assert_eq!(resolutions.len(), 5);
    let resume_snapshots = directory.join("resume-snapshots");
    let resume_output = directory.join("resume-output.json");
    let mut snapshot = directory.join("snapshots/snapshot-0000.json");
    for (index, resolution) in resolutions.iter().enumerate() {
        fs::write(
            &resolution_path,
            serde_json::to_vec(resolution).expect("resolution serializes"),
        )
        .expect("resolution writes");
        let status = aster()
            .args([
                "resume",
                example("main.aster").to_str().expect("UTF-8 path"),
                "--snapshot",
                snapshot.to_str().expect("UTF-8 path"),
                "--resolution",
                resolution_path.to_str().expect("UTF-8 path"),
                "--trace",
                resume_trace.to_str().expect("UTF-8 path"),
                "--snapshot-dir",
                resume_snapshots.to_str().expect("UTF-8 path"),
                "--output-state",
                resume_output.to_str().expect("UTF-8 path"),
            ])
            .status()
            .expect("resume executes");
        assert!(status.success(), "resume step {index} succeeds");
        snapshot = resume_snapshots.join("resume-next.json");
    }
    assert_eq!(
        fs::read(directory.join("record.json")).expect("recorded state reads"),
        fs::read(&resume_output).expect("resumed state reads")
    );

    let replay_output = directory.join("resume-replay.json");
    let status = aster()
        .args([
            "replay",
            example("main.aster").to_str().expect("UTF-8 path"),
            "--trace",
            resume_trace.to_str().expect("UTF-8 path"),
            "--input",
            example("event.json").to_str().expect("UTF-8 path"),
            "--state",
            example("initial-state.json").to_str().expect("UTF-8 path"),
            "--capabilities",
            example("capabilities.json").to_str().expect("UTF-8 path"),
            "--output-state",
            replay_output.to_str().expect("UTF-8 path"),
        ])
        .status()
        .expect("resumed trace replay executes");
    assert!(status.success());
    assert_eq!(
        fs::read(resume_output).expect("resumed state reads"),
        fs::read(replay_output).expect("replayed resumed state reads")
    );
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
    assert_resume(directory.path());
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
    let source_json = aster()
        .args([
            "check",
            bad_source.to_str().expect("UTF-8 path"),
            "--diagnostic-format",
            "json",
        ])
        .output()
        .expect("JSON source check executes");
    assert_eq!(source_json.status.code(), Some(1));
    assert!(source_json.stderr.is_empty());
    let diagnostics: serde_json::Value =
        serde_json::from_slice(&source_json.stdout).expect("diagnostics are stdout JSON");
    assert_eq!(diagnostics[0]["code"], "ASTER-TYPE-2001");

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

#[test]
fn malformed_json_trace_and_snapshot_are_controlled_cli_failures() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let malformed = directory.path().join("malformed.json");
    fs::write(&malformed, b"{not-json").expect("malformed input writes");

    let mut runtime_args = run_args(directory.path());
    let input_index = runtime_args
        .iter()
        .position(|value| value == "--input")
        .expect("input argument")
        + 1;
    runtime_args[input_index] = malformed.display().to_string();
    let runtime = aster()
        .args(runtime_args)
        .status()
        .expect("malformed event is controlled");
    assert_eq!(runtime.code(), Some(2));

    let replay = aster()
        .args([
            "replay",
            example("main.aster").to_str().expect("UTF-8 path"),
            "--trace",
            malformed.to_str().expect("UTF-8 path"),
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
        .expect("malformed trace is controlled");
    assert_eq!(replay.code(), Some(3));

    let resume = aster()
        .args([
            "resume",
            example("main.aster").to_str().expect("UTF-8 path"),
            "--snapshot",
            malformed.to_str().expect("UTF-8 path"),
            "--resolution",
            malformed.to_str().expect("UTF-8 path"),
            "--trace",
            malformed.to_str().expect("UTF-8 path"),
            "--snapshot-dir",
            directory.path().to_str().expect("UTF-8 path"),
            "--output-state",
            directory
                .path()
                .join("resume.json")
                .to_str()
                .expect("UTF-8 path"),
        ])
        .status()
        .expect("malformed snapshot is controlled");
    assert_eq!(resume.code(), Some(2));
}
