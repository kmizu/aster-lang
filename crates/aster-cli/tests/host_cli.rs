use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use serde_json::{Value, json};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example(name: &str) -> PathBuf {
    root().join("examples/meeting-scheduler").join(name)
}

fn governed_note_example(name: &str) -> PathBuf {
    root().join("examples/governed-note").join(name)
}

fn host_args(directory: &Path) -> Vec<String> {
    vec![
        "host".to_owned(),
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
        "--trace".to_owned(),
        directory.join("run.trace.jsonl").display().to_string(),
        "--snapshot-dir".to_owned(),
        directory.join("snapshots").display().to_string(),
        "--output-state".to_owned(),
        directory.join("output.json").display().to_string(),
    ]
}

fn spawn(args: &[String]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_aster"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("host starts")
}

fn governed_note_host_args(directory: &Path) -> Vec<String> {
    vec![
        "host".to_owned(),
        governed_note_example("main.aster").display().to_string(),
        "--agent".to_owned(),
        "NoteKeeper".to_owned(),
        "--event".to_owned(),
        "message".to_owned(),
        "--input".to_owned(),
        governed_note_example("event.json").display().to_string(),
        "--state".to_owned(),
        governed_note_example("initial-state.json")
            .display()
            .to_string(),
        "--capabilities".to_owned(),
        governed_note_example("capabilities.json")
            .display()
            .to_string(),
        "--trace".to_owned(),
        directory.join("record.trace.jsonl").display().to_string(),
        "--snapshot-dir".to_owned(),
        directory.join("snapshots").display().to_string(),
        "--output-state".to_owned(),
        directory.join("record-state.json").display().to_string(),
    ]
}

fn read_frame(reader: &mut impl BufRead) -> Value {
    let mut line = String::new();
    reader.read_line(&mut line).expect("protocol line reads");
    assert!(line.ends_with('\n'), "protocol frame is newline terminated");
    serde_json::from_str(&line).expect("protocol line is one JSON object")
}

fn send_frame(writer: &mut impl Write, frame: &Value) {
    serde_json::to_writer(&mut *writer, &frame).expect("reply serializes");
    writer.write_all(b"\n").expect("reply newline writes");
    writer.flush().expect("reply flushes");
}

fn reply(frame: &Value, kind: &str, payload: &Value) -> Value {
    json!({
        "schema_version": 1,
        "session_id": frame["session_id"],
        "in_reply_to": frame["message_id"],
        "kind": kind,
        "payload": payload,
    })
}

#[test]
fn host_stdout_contains_protocol_json_only() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut child = spawn(&host_args(directory.path()));
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
    let hello = read_frame(&mut stdout);
    assert_eq!(hello["kind"], "hello");
    send_frame(
        child.stdin.as_mut().expect("stdin"),
        &reply(
            &hello,
            "hello_ack",
            &json!({"protocol": "aster-host", "protocol_version": 1}),
        ),
    );
    let preview = read_frame(&mut stdout);
    assert_eq!(preview["kind"], "effect_preview");
    child.kill().expect("test host stops");
    let _ = child.wait().expect("test host reaped");
}

#[test]
fn host_resume_reemits_the_durable_execution_grant() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut child = spawn(&host_args(directory.path()));
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
    let hello = read_frame(&mut stdout);
    send_frame(
        child.stdin.as_mut().expect("stdin"),
        &reply(
            &hello,
            "hello_ack",
            &json!({"protocol": "aster-host", "protocol_version": 1}),
        ),
    );
    let preview = read_frame(&mut stdout);
    send_frame(
        child.stdin.as_mut().expect("stdin"),
        &reply(
            &preview,
            "effect_admission",
            &json!({
                "request_hash": preview["payload"]["request"]["request_hash"],
                "max_usage": {"model_tokens": 100},
            }),
        ),
    );
    let original_grant = read_frame(&mut stdout);
    assert_eq!(original_grant["kind"], "execute_grant");
    child.kill().expect("test host stops after grant");
    let _ = child.wait().expect("test host reaped");

    let resume_args = vec![
        "host-resume".to_owned(),
        example("main.aster").display().to_string(),
        "--snapshot".to_owned(),
        directory
            .path()
            .join("snapshots/snapshot-0000.json")
            .display()
            .to_string(),
        "--trace".to_owned(),
        directory
            .path()
            .join("run.trace.jsonl")
            .display()
            .to_string(),
        "--snapshot-dir".to_owned(),
        directory
            .path()
            .join("resume-snapshots")
            .display()
            .to_string(),
        "--output-state".to_owned(),
        directory
            .path()
            .join("resume-output.json")
            .display()
            .to_string(),
    ];
    let mut resumed = spawn(&resume_args);
    let mut resumed_stdout = BufReader::new(resumed.stdout.take().expect("stdout"));
    let hello = read_frame(&mut resumed_stdout);
    send_frame(
        resumed.stdin.as_mut().expect("stdin"),
        &reply(
            &hello,
            "hello_ack",
            &json!({"protocol": "aster-host", "protocol_version": 1}),
        ),
    );
    let resumed_grant = read_frame(&mut resumed_stdout);
    assert_eq!(resumed_grant["kind"], "execute_grant");
    for field in [
        "request",
        "max_usage",
        "snapshot_hash",
        "execution_grant_hash",
    ] {
        assert_eq!(
            resumed_grant["payload"][field], original_grant["payload"][field],
            "{field} must survive crash/resume"
        );
    }
    resumed.kill().expect("resumed test host stops");
    let _ = resumed.wait().expect("resumed test host reaped");
}

#[test]
fn malformed_unknown_version_utf8_and_eof_fail_with_protocol_only_stdout() {
    for index in 0..5 {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut child = spawn(&host_args(directory.path()));
        let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
        let hello = read_frame(&mut stdout);
        let session_id = hello["session_id"].as_str().expect("session id");
        let bytes = match index {
            0 => Some(b"{\n".to_vec()),
            1 => Some(vec![0xff, b'\n']),
            2 => Some(
                format!(
                    "{{\"schema_version\":1,\"session_id\":\"{session_id}\",\"in_reply_to\":0,\"kind\":\"hello_ack\",\"payload\":{{\"protocol\":\"aster-host\",\"protocol_version\":1,\"unexpected\":true}}}}\n"
                )
                .into_bytes(),
            ),
            3 => Some(
                format!(
                    "{{\"schema_version\":2,\"session_id\":\"{session_id}\",\"in_reply_to\":0,\"kind\":\"hello_ack\",\"payload\":{{\"protocol\":\"aster-host\",\"protocol_version\":1}}}}\n"
                )
                .into_bytes(),
            ),
            _ => None,
        };
        if let Some(bytes) = bytes {
            child
                .stdin
                .as_mut()
                .expect("stdin")
                .write_all(&bytes)
                .expect("bad reply writes");
        }
        drop(child.stdin.take());
        let mut remaining = String::new();
        stdout
            .read_to_string(&mut remaining)
            .expect("remaining protocol output reads");
        let status = child.wait().expect("host exits");
        assert_eq!(status.code(), Some(2), "case {index} exits as host failure");
        let frames = remaining
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("stdout line is JSON"))
            .collect::<Vec<_>>();
        assert_eq!(frames.len(), 1, "case {index} has one terminal frame");
        assert_eq!(frames[0]["kind"], "failed", "case {index} terminates");
    }
}

#[test]
fn host_redaction_keeps_private_and_secret_values_out_of_all_outputs() {
    const PRIVATE: &str = "PRIVATE_FRAME_VALUE";
    const SECRET: &str = "SECRET_FRAME_VALUE";
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut child = spawn(&host_args(directory.path()));
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
    let hello = read_frame(&mut stdout);
    let hostile = format!(
        "{{\"schema_version\":1,\"session_id\":\"{}\",\"in_reply_to\":0,\"kind\":\"hello_ack\",\"payload\":{{\"protocol\":\"aster-host\",\"protocol_version\":1,\"private\":\"{PRIVATE}\",\"secret\":\"{SECRET}\"}}}}\n",
        hello["session_id"].as_str().expect("session id")
    );
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(hostile.as_bytes())
        .expect("hostile frame writes");
    drop(child.stdin.take());
    let mut remaining_stdout = String::new();
    stdout
        .read_to_string(&mut remaining_stdout)
        .expect("terminal frame reads");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr)
        .expect("stderr reads");
    let status = child.wait().expect("host exits");
    assert_eq!(status.code(), Some(2));

    let mut artifacts = remaining_stdout;
    artifacts.push_str(&stderr);
    for path in [
        directory.path().join("run.trace.jsonl"),
        directory.path().join("snapshots/snapshot-0000.json"),
        directory.path().join("output.json"),
    ] {
        if path.exists() {
            artifacts.push_str(&String::from_utf8_lossy(
                &std::fs::read(path).expect("artifact reads"),
            ));
        }
    }
    assert!(!artifacts.contains(PRIVATE));
    assert!(!artifacts.contains(SECRET));
}

fn execute_governed_effect(grant: &Value, note_path: &Path) -> Value {
    let identity = grant["payload"]["request"]["identity"]
        .as_str()
        .expect("effect identity");
    match identity {
        "DraftNote" => json!({"content": "ship v0.2\n"}),
        "Workspace.fetch" | "Workspace.lookup" => {
            json!(std::fs::read_to_string(note_path).expect("note reads"))
        }
        "NotePolicy" => json!({"approved": true}),
        "Workspace.store" => {
            let content = grant["payload"]["request"]["payload"]["arguments"]["content"]
                .as_str()
                .expect("write content");
            std::fs::write(note_path, content).expect("granted note writes");
            json!(content)
        }
        other => panic!("unexpected effect identity {other}"),
    }
}

fn run_governed_note_host(directory: &Path, note_path: &Path) -> Vec<String> {
    let mut child = spawn(&governed_note_host_args(directory));
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
    let hello = read_frame(&mut stdout);
    send_frame(
        child.stdin.as_mut().expect("stdin"),
        &reply(
            &hello,
            "hello_ack",
            &json!({"protocol": "aster-host", "protocol_version": 1}),
        ),
    );

    let mut effect_kinds = Vec::new();
    loop {
        let preview = read_frame(&mut stdout);
        if preview["kind"] == "completed" {
            break;
        }
        assert_eq!(preview["kind"], "effect_preview");
        let request = &preview["payload"]["request"];
        let kind = request["kind"].as_str().expect("effect kind");
        effect_kinds.push(kind.to_owned());
        if kind == "write" {
            assert_eq!(
                std::fs::read_to_string(note_path).expect("preview note reads"),
                "before\n",
                "a preview must not authorize filesystem mutation"
            );
        }
        let max_usage = if kind == "model" {
            json!({"model_tokens": 100})
        } else {
            json!({})
        };
        send_frame(
            child.stdin.as_mut().expect("stdin"),
            &reply(
                &preview,
                "effect_admission",
                &json!({
                    "request_hash": request["request_hash"],
                    "max_usage": max_usage,
                }),
            ),
        );

        let grant = read_frame(&mut stdout);
        assert_eq!(grant["kind"], "execute_grant");
        assert_eq!(grant["payload"]["request"], *request);
        let payload = execute_governed_effect(&grant, note_path);
        let actual_usage = if kind == "model" {
            json!({"model_tokens": 12})
        } else {
            json!({})
        };
        send_frame(
            child.stdin.as_mut().expect("stdin"),
            &reply(
                &grant,
                "effect_resolution",
                &json!({
                    "request_hash": grant["payload"]["request"]["request_hash"],
                    "execution_grant_hash": grant["payload"]["execution_grant_hash"],
                    "payload": payload,
                    "actual_usage": actual_usage,
                }),
            ),
        );
    }
    drop(child.stdin.take());
    let status = child.wait().expect("host exits");
    assert!(status.success(), "host exits successfully: {status}");
    effect_kinds
}

fn replay_governed_note(directory: &Path) -> PathBuf {
    let replay_state = directory.join("replay-state.json");
    let replay = Command::new(env!("CARGO_BIN_EXE_aster"))
        .args([
            "replay",
            governed_note_example("main.aster")
                .to_str()
                .expect("source path"),
            "--trace",
            directory
                .join("record.trace.jsonl")
                .to_str()
                .expect("trace path"),
            "--input",
            governed_note_example("event.json")
                .to_str()
                .expect("event path"),
            "--state",
            governed_note_example("initial-state.json")
                .to_str()
                .expect("state path"),
            "--capabilities",
            governed_note_example("capabilities.json")
                .to_str()
                .expect("capabilities path"),
            "--output-state",
            replay_state.to_str().expect("replay output path"),
        ])
        .status()
        .expect("replay starts");
    assert!(replay.success(), "replay exits successfully: {replay}");
    replay_state
}

#[test]
fn codex_style_host_executes_governed_note_then_replays_byte_identically() {
    let directory = tempfile::tempdir().expect("temporary workspace");
    let note_path = directory.path().join("note.txt");
    std::fs::write(&note_path, "before\n").expect("initial note writes");

    let effect_kinds = run_governed_note_host(directory.path(), &note_path);
    assert_eq!(effect_kinds, ["model", "read", "approval", "write", "read"]);
    assert_eq!(
        std::fs::read_to_string(&note_path).expect("final note reads"),
        "ship v0.2\n"
    );

    let replay_state = replay_governed_note(directory.path());
    assert_eq!(
        std::fs::read(directory.path().join("record-state.json")).expect("record state reads"),
        std::fs::read(replay_state).expect("replay state reads")
    );
}
