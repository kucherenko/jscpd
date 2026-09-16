//! What a run reports.
//!
//! The types themselves live in [`cpd_core::deadcode`] so that the reporters
//! can render a dead-code run without depending on basta. This module is the
//! name analysis code uses for them.

pub use cpd_core::deadcode::{CategoryCount, ConfidenceLevel, Finding, Reason, Report, Stats};
