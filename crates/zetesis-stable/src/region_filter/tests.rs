//! The region check composes with candidate coverage on every route and never
//! with a frozen-reduct query. Source injections exist only in this test module.

mod consequences;
mod filters;
mod positive;
mod support;

pub(crate) mod injection;
