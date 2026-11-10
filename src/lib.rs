//! Checker for the Remit contract language, version 0. See SPEC.md.

pub mod ast;
pub mod check;
pub mod diag;
pub mod explain;
pub mod lexer;
pub mod list;
pub mod parser;

pub use check::{load, World};
pub use diag::Diag;
