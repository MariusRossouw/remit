//! Checker for the Remit contract language, version 0. See SPEC.md.

pub mod ast;
pub mod check;
pub mod diag;
pub mod explain;
pub mod gate;
pub mod journal;
pub mod json;
pub mod lexer;
pub mod list;
pub mod parser;
pub mod proxy;

pub use check::{load, World};
pub use diag::Diag;
