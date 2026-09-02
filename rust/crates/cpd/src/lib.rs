//! The `jscpd` crate is the CLI binary (`jscpd` / `cpd`). This library target
//! exists so integration tests can share a few helpers; it is not a public API
//! and may change without notice. For programmatic use see the `cpd-finder`,
//! `cpd-core`, `cpd-tokenizer` and `cpd-reporter` crates.
#![doc = include_str!("../README.md")]

pub mod timer;
