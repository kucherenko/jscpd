//! basta — dead code detection.

pub mod analyze;
pub mod classify;
pub mod cli;
pub mod confidence;
pub mod config;
pub mod entry;
pub mod finding;
pub mod graph;
pub mod lang;
pub mod model;
pub mod resolve;
pub mod run;

#[cfg(test)]
pub(crate) mod test_scan;
