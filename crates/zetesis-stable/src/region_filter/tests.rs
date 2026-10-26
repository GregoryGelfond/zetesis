//! The region check composes with candidate coverage on every route and never
//! with a frozen-reduct query. Test filters implement the crate's own seam.

mod consequences;
mod filters;
mod positive;
mod support;
