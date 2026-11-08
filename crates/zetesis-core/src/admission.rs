//! Shared limits interpreted by semantic admission owners.

const DEFAULT_MAX_TEMPLATES: usize = 100_000;
const DEFAULT_MAX_PREDICATE_ARITY: usize = 32;
const DEFAULT_MAX_VARIABLES: usize = 64;
const DEFAULT_MAX_POSITIVE_BODY: usize = 1_024;
const DEFAULT_MAX_DOMAIN_VALUES: usize = 1_000_000;
// The same independent named-storage default as core relations and CPU closure.
const DEFAULT_MAX_BYTES: usize = 128 * 1024 * 1024;

/// Template-level admission budgets. Zero is a real limit, never unlimited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmissionLimits {
    /// Maximum number of source-normalized rules: templates and fact occurrences.
    pub max_templates: usize,
    /// Maximum predicate arity.
    pub max_predicate_arity: usize,
    /// Maximum distinct variables in any one template.
    pub max_variables_per_template: usize,
    /// Maximum ordinary positive antecedents per template.
    pub max_positive_body: usize,
    /// Maximum distinct values appearing anywhere, including filters/gates.
    pub max_domain_values: usize,
    /// Inclusive canonical storage and admitted metadata allowance, including
    /// indexes, vector capacity, admission scratch and publication overlap.
    /// Caller-supplied construction descriptions, allocator bookkeeping and Arc
    /// reference counters are excluded. The default is 128 MiB, independently
    /// of the template/domain count limits; zero is a real allowance.
    pub max_bytes: usize,
}
impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_templates: DEFAULT_MAX_TEMPLATES,
            max_predicate_arity: DEFAULT_MAX_PREDICATE_ARITY,
            max_variables_per_template: DEFAULT_MAX_VARIABLES,
            max_positive_body: DEFAULT_MAX_POSITIVE_BODY,
            max_domain_values: DEFAULT_MAX_DOMAIN_VALUES,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}
