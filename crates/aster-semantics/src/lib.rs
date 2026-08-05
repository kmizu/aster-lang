#![forbid(unsafe_code)]

//! Deterministic ASTER name, type, effect, capability, taint, and affine checks.

mod checker;
mod expression;
mod model;
mod types;

pub use checker::{CheckedProgram, check, check_source};
pub use types::Type;
