use std::{fs, path::PathBuf};

use aster_ir::{InstructionKind, Program, Stage, lower};
use aster_semantics::check_source;
use aster_syntax::SourceFile;

fn meeting_program() -> Program {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("examples/meeting-scheduler/main.aster");
    let text = fs::read_to_string(&path).expect("meeting example is readable");
    let checked = check_source(&SourceFile::new(path.display().to_string(), text))
        .expect("meeting example checks");
    lower(&checked).expect("meeting example lowers")
}

#[test]
fn lowering_has_stable_ids_hash_and_json_round_trip() {
    // Catches source-location or map-order leakage into persisted IR identity.
    let first = meeting_program();
    let second = meeting_program();

    assert_eq!(first, second);
    assert_eq!(first.program_hash.len(), 64);
    let json = first.to_json().expect("IR serializes");
    let decoded = Program::from_json(&json).expect("IR deserializes and validates");
    assert_eq!(decoded, first);

    let handler = first
        .handler("Scheduler", "message")
        .expect("handler exists");
    for (index, instruction) in handler.instructions.iter().enumerate() {
        assert_eq!(instruction.id.index(), u32::try_from(index).unwrap());
    }
}

#[test]
fn meeting_governance_stages_are_explicit_and_ordered() {
    // Catches hidden effects inside a generic AST evaluation instruction.
    let program = meeting_program();
    let handler = program.handler("Scheduler", "message").unwrap();
    let stages: Vec<_> = handler
        .instructions
        .iter()
        .filter_map(|instruction| instruction.kind.stage())
        .collect();

    assert_eq!(
        stages,
        vec![
            Stage::Inference,
            Stage::Validation,
            Stage::Observation,
            Stage::Intent,
            Stage::Proposal,
            Stage::Authorization,
            Stage::Commit,
            Stage::Observation,
            Stage::Reconciliation,
            Stage::StateUpdate,
            Stage::Return,
        ]
    );
    assert!(handler.instructions.iter().all(|instruction| {
        !matches!(instruction.kind, InstructionKind::Evaluate { .. })
            || instruction.kind.stage().is_none()
    }));
}

#[test]
fn source_control_flow_lowers_to_explicit_branch_targets() {
    // Catches recursive AST execution hidden behind the IR boundary.
    let source = SourceFile::new(
        "branch.aster",
        r"module branch;
fn choose(flag: Bool) -> Unit {
  if flag { return Unit; } else { return Unit; };
  return Unit;
}
",
    );
    let checked = check_source(&source).expect("branch program checks");
    let program = lower(&checked).expect("branch program lowers");
    let routine = program.routine("choose").expect("routine exists");

    assert!(
        routine
            .instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, InstructionKind::Branch { .. }))
    );
    assert!(
        routine
            .instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, InstructionKind::Jump { .. }))
    );
}
