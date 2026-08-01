include!("lib.rs");

mod revision;
pub use revision::*;
mod revision_retry;

#[cfg(test)]
mod revision_tests;
