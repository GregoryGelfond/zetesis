//! Helpers the zetesis workspace's tests share, over `zetesis-core` alone.
//!
//! Several crates' tests use these helpers, so each is implemented once here.
//! The crate depends on nothing above `zetesis-core`, so the unit tests of
//! the crates that use it never link a second copy of themselves. It is not
//! published or installed.

pub mod io;
pub mod programs;
