//! Extended crate root used to compose the established UniFFI surface with Milestone 9.3.

include!("lib.rs");

mod revision;
pub use revision::*;
