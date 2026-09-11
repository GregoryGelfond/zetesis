//! Reporting-local serialization of native bounds; kernels do not depend on serde.

use serde::Serialize;

#[derive(Serialize)]
#[serde(remote = "zetesis_themelios::BundleLimits")]
#[expect(
    clippy::struct_field_names,
    reason = "Serde must access BundleLimits by its native max_ ceiling field names."
)]
pub(super) struct Bundle {
    max_roots: usize,
    max_files: usize,
    max_file_bytes: usize,
    max_total_bytes: usize,
    max_include_depth: usize,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_themelios::BundleAdmissionOptions")]
pub(super) struct Admission {
    max_syntax_nodes: usize,
    max_syntax_depth: usize,
    max_body_elements: usize,
    #[serde(with = "Core")]
    core_limits: zetesis_core::AdmissionLimits,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_core::AdmissionLimits")]
#[expect(
    clippy::struct_field_names,
    reason = "Serde must access zetesis_core::AdmissionLimits by its native max_ ceiling field names."
)]
struct Core {
    max_templates: usize,
    max_predicate_arity: usize,
    max_variables_per_template: usize,
    max_positive_body: usize,
    max_domain_values: usize,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_themelios::ExpansionLimits")]
#[expect(
    clippy::struct_field_names,
    reason = "Serde must access ExpansionLimits by its native max_ ceiling field names."
)]
pub(super) struct Expansion {
    max_constants: usize,
    max_term_work: usize,
    max_templates: usize,
    max_values: usize,
    max_scalar_bytes: usize,
    max_origin_locations: usize,
    max_metadata_statements: usize,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_themelios::FormulaLimits")]
pub(super) struct Formula {
    max_objective_presence_entries: usize,
    max_domain_values: usize,
    max_assignment_values: usize,
    max_disjunction_elements: usize,
    max_support_index_entries: usize,
    max_support_bytes: usize,
    max_aggregate_cache_rows: usize,
    max_aggregate_cache_key_bytes: usize,
    max_aggregate_cache_elements: usize,
    max_aggregate_cache_roots: usize,
    max_analysis_nodes: usize,
    max_analysis_edges: usize,
    max_substitutions: u64,
    max_work: u64,
    max_support_rounds: u64,
    max_origin_locations: usize,
    #[serde(with = "Theory")]
    theory: zetesis_ferraris::AdmissionLimits,
    #[serde(with = "Aggregate")]
    aggregate: zetesis_ferraris::AggregateLimits,
    #[serde(with = "Objective")]
    objective: zetesis_objective::AdmissionLimits,
    #[serde(with = "Observation")]
    observation: zetesis_themelios::observation::AdmissionLimits,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_ferraris::AdmissionLimits")]
#[expect(
    clippy::struct_field_names,
    reason = "Serde must access zetesis_ferraris::AdmissionLimits by its native max_ ceiling field names."
)]
struct Theory {
    max_atoms: usize,
    max_nodes: usize,
    max_roots: usize,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_ferraris::AggregateLimits")]
#[expect(
    clippy::struct_field_names,
    reason = "Serde must access AggregateLimits by its native max_ ceiling field names."
)]
struct Aggregate {
    max_elements: usize,
    max_nodes: usize,
    max_work: u64,
    max_states: usize,
    max_subsets: u64,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_objective::AdmissionLimits")]
#[expect(
    clippy::struct_field_names,
    reason = "Serde must access zetesis_objective::AdmissionLimits by its native max_ ceiling field names."
)]
struct Objective {
    max_templates: usize,
    max_tuple_width: usize,
    max_variables_per_template: usize,
    max_positive_body: usize,
    max_predicate_arity: usize,
    max_filters: usize,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_themelios::observation::AdmissionLimits")]
#[expect(
    clippy::struct_field_names,
    reason = "Serde must access observation::AdmissionLimits by its native max_ ceiling field names."
)]
struct Observation {
    max_directives: u32,
    max_nodes: u32,
    max_depth: u32,
    max_bytes: u32,
    max_variables: u32,
    max_body_elements: u32,
    max_arity: u32,
    max_origins: u32,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_sat::Limits")]
pub(super) struct Search {
    #[serde(with = "Cnf")]
    admission: zetesis_sat::AdmissionLimits,
    #[serde(with = "SearchWork")]
    #[expect(
        clippy::struct_field_names,
        reason = "Serde reads native Limits.search, which distinguishes search work from admission bounds."
    )]
    search: zetesis_sat::SearchLimits,
    max_candidates: u64,
    max_verification_work: u64,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_sat::AdmissionLimits")]
#[expect(
    clippy::struct_field_names,
    reason = "Serde must access zetesis_sat::AdmissionLimits by its native max_ ceiling field names."
)]
struct Cnf {
    max_variables: usize,
    max_clauses: usize,
    max_literals: usize,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_sat::SearchLimits")]
struct SearchWork {
    max_work: u64,
    max_decisions: u64,
}

#[derive(Serialize)]
#[serde(remote = "zetesis_ferraris::TightPlanLimits")]
#[expect(
    clippy::struct_field_names,
    reason = "Serde must access TightPlanLimits by its native max_ ceiling field names."
)]
pub(super) struct Certificate {
    max_producers: usize,
    max_dependencies: usize,
    max_bytes: u64,
    max_work: u64,
}
