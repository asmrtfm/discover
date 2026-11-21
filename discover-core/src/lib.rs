//! The language-independent half of every discover-* navigator. A language
//! crate supplies parsing, module resolution and entry points through
//! [`graph::Lang`]; everything built on top of those — cached liveness, the
//! importers index, dead files, trace, prune and the shared subcommands —
//! lives here.

pub mod cache;
pub mod cli;
pub mod commands;
pub mod common;
pub mod graph;
pub mod help;
pub mod memo;
pub mod par;
pub mod project;
