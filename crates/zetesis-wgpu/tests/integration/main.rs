//! Integration tests of `zetesis-wgpu`, compiled as one test binary; each
//! module was one test target.

mod support;
mod formula;
mod formula_interface;
mod hardware;
mod hardware_aggregate;
mod hardware_context;
mod hardware_formula;
mod hardware_lazy;
mod hardware_relation;
mod hardware_tight;
