//! Integration tests of `zetesis-cpu`, compiled as one test binary; each
//! module was one test target.

mod support;
mod cancellation;
mod candidate_restrictions;
mod carrier_bounds;
mod closure_storage;
mod closure_transfer;
mod conformance;
mod delta_join;
mod lazy_failures;
mod lazy_rounds;
mod lazy_worlds;
mod ordered_joins;
mod region_split;
mod regions;
mod runtime;
mod seed_selections;
mod seed_views;
mod semantic_receipts;
mod shared_batch;
mod static_oracle;
mod view_counts;
