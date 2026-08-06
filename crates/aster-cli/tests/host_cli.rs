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
