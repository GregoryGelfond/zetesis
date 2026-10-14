//! Test support for the compiler: a compiler over an empty source, for tests
//! of compiler-internal steps; the one constructor of a comparison literal,
//! for tests that build one over their own catalog; and the switch that
//! compiles facts as rules, for tests that compare fact blocks with the rule
//! form.

use themelios_base::source::{Source, SourceId};
use themelios_program::program::Relation;

use super::{Budget, Comparison, Compiler, Counters, Expression, SupportCatalog, domain};
use crate::formula_support::components;
use crate::{AdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits, ProgramSite};

pub(crate) use super::facts::testing::with_facts_as_rules;
pub(crate) use super::integer::WithoutForm;

/// The comparison `left relation right`, compiled with its integer comparison
/// form through the compiler's one constructor, reading its constants through
/// `source` as the compiler does.
///
/// # Errors
/// The constructor's refusals.
pub(crate) fn comparison(
    parts: (Expression, Relation, Expression),
    source: &components::Admission<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<Comparison, FormulaFailure> {
    super::comparison::compile(parts, source, limits, counters, budget, location)
}

/// Run `work` with a compiler whose expansion budget has `limits`.
pub(super) fn with_compiler<R>(
    limits: ExpansionLimits,
    work: impl FnOnce(&mut Compiler<'_>) -> R,
) -> R {
    let source = Source::new(SourceId::new(113), String::new()).unwrap();
    let mut budget = Budget::new(limits, 100);
    let formula_limits = FormulaLimits::default();
    let location = ProgramSite::source(themelios_base::span::Location {
        source: source.id(),
        span: source.span(),
    });
    let mut catalog = SupportCatalog::default();
    let mut counters = Counters::default();
    let admission = catalog
        .component_admission(&formula_limits, &mut counters, location)
        .unwrap();
    let domain = domain::Domain::new(&admission, &formula_limits, &counters, location).unwrap();
    let mut compiler = Compiler {
        options: AdmissionOptions::default().into(),
        limits: &formula_limits,
        budget: &mut budget,
        source: admission,
        counters: &mut counters,
        domain,
        next_aggregate: 0,
        dependency_projection: false,
        location: ProgramSite::source(themelios_base::span::Location {
            source: source.id(),
            span: source.span(),
        }),
    };
    work(&mut compiler)
}
