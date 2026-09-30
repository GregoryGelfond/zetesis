//! Helpers the zetesis workspace's tests share over the Ferraris theories.
//!
//! The crate depends on `zetesis-core`, `zetesis-cpu` and `zetesis-ferraris`
//! alone, so the tests that use it compile nothing beyond them: the device
//! tests of `zetesis-wgpu` and the integration tests of `zetesis-sat` and
//! `zetesis-ferraris`. It is not published or installed.

pub mod aggregate;
pub mod theories;
