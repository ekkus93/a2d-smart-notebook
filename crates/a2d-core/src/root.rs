//! Extended crate root used to compose the established core with Milestone 9.3.

include!("lib.rs");

mod revision;
pub use revision::*;

#[cfg(test)]
mod revision_tests;
