//! Bounded, demonstrably pool-free input to the established themelios analysis.
//!
//! Limits count structure and potential dependency occurrences before upstream
//! cloning/unpooling. Signature payload is charged separately: a small graph
//! with long predicate names can otherwise have a large byte footprint. These
//! are logical admission allowances, not allocator or peak-RSS measurements.

use themelios_base::span::Location;
use themelios_program::program::{Arguments, Atom, Body, Program, Rule, Statement};
use themelios_program::provenance::WithProvenance;
use themelios_program::symbol::{Name, Sign, Signature, Symbol};
use themelios_program::term::Term;
use themelios_program::transform::Visit;
use zetesis_core::{AtomPattern, Value};

use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::{
    ExpansionResource, FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature, extended,
};

/// Rebuild one already expanded fact. The caller bounds fact count, scalar
/// expansion and provenance before calling. Only the enclosing statement keeps
/// parsed origins; synthesized children truthfully have constructed provenance.
pub(crate) fn fact(
    pattern: &AtomPattern,
    source: &WithProvenance<Statement>,
    location: Location,
) -> Result<WithProvenance<Statement>, FormulaFailure> {
    let terms = pattern
        .terms()
        .iter()
        .map(|term| {
            let zetesis_core::Term::Constant(value) = term else {
                unreachable!("expanded fact is ground")
            };
            Ok(Term::Symbolic(match value {
                Value::Infimum => Symbol::Infimum,
                Value::Supremum => Symbol::Supremum,
                Value::Number(number) => Symbol::Number(*number),
                Value::Structured(value) => {
                    crate::structural_value::to_symbol(value).map_err(|error| {
                        crate::AdmissionFailure::Construction {
                            error: zetesis_core::ConstructionError::Value(match error {
                                crate::structural_value::BridgeError::Allocation => {
                                    zetesis_core::ValueError::Allocation
                                }
                                crate::structural_value::BridgeError::InvalidName => {
                                    zetesis_core::ValueError::Shape
                                }
                            }),
                            location,
                        }
                    })?
                }
                Value::String(value) => Symbol::String(value.clone()),
                Value::Symbol(value) => Symbol::Function {
                    name: Name::new(value.clone()).expect("validated source symbol"),
                    arguments: Vec::new(),
                    sign: Sign::Positive,
                },
            }))
        })
        .collect::<Result<Vec<_>, FormulaFailure>>()?;
    let atom = Atom {
        sign: crate::coherence::source_sign(pattern.predicate().sign()),
        name: Name::new(pattern.predicate().name()).expect("validated source predicate"),
        arguments: Arguments::Single(terms),
    };
    Ok(WithProvenance::new(
        Statement::Rule(Rule::new(atom, Body::new([]))),
        source.provenance().clone(),
    ))
}

pub(crate) fn analyze(
    program: &Program,
    limits: &FormulaLimits,
    budget: &mut Budget,
    fallback: Location,
) -> Result<themelios_analysis::Analysis, FormulaFailure> {
    let mut nodes = 0_u128;
    let mut edges = 0_u128;
    for source in program.statements() {
        let location = extended::origin(source, fallback);
        // The analyzed projection contains rules and optimization statements.
        // A future directive must add its graph/allocation contract explicitly;
        // in particular, upstream #external pseudo-rules also allocate edges.
        if !matches!(source.get(), Statement::Rule(_) | Statement::Optimize(_)) {
            return Err(unsupported(ProfileFeature::Statement, location).into());
        }
        let mut scan = PoolFree::default();
        scan.visit_statement(source.get());
        if scan.pooled {
            return Err(unsupported(ProfileFeature::AnalysisPool, location).into());
        }
        nodes = nodes.saturating_add(scan.nodes).saturating_add(1);
        ceiling(
            FormulaResource::AnalysisNodes,
            nodes,
            limits.max_analysis_nodes as u128,
            location,
        )?;
        budget.charge(
            ExpansionResource::TermWork,
            scan.nodes.saturating_add(1),
            location,
        )?;
        // An occurrence contributes at most two signature modes (aggregate and
        // negative). Reserve that payload before the allocating accessors below.
        budget.charge(
            ExpansionResource::ScalarBytes,
            scan.name_bytes.saturating_mul(2),
            location,
        )?;
        if let Statement::Rule(rule) = source.get() {
            // These upstream accessors include aggregate and head-choice conditions.
            // Their per-rule vectors are bounded by the preceding structural scan.
            let (heads, head_bytes) = signature_sizes(rule.head_signatures());
            let (dependencies, dependency_bytes) =
                signature_sizes(rule.body_signatures().map(|(_, signature)| signature));
            let added = heads.saturating_mul(dependencies);
            edges = edges.saturating_add(added);
            ceiling(
                FormulaResource::AnalysisEdges,
                edges,
                limits.max_analysis_edges as u128,
                location,
            )?;
            budget.charge(ExpansionResource::TermWork, added, location)?;
            // Bound submitted graph endpoint payload before Analysis::of builds
            // edges. Duplicate graph keys still consume preflight allowance.
            let payload = head_bytes
                .saturating_mul(dependencies)
                .saturating_add(dependency_bytes.saturating_mul(heads));
            budget.charge(ExpansionResource::ScalarBytes, payload, location)?;
        }
    }
    // Analysis::of internally unpools. The visitor above independently certifies
    // that neither term pools nor pooled atom argument lists reach that operation.
    // Retain the exact upstream facts. The pinned tier does not classify clingo
    // aggregate equality guards as binders; its safety reading can therefore be
    // narrower than our separately checked source profile. Source admission's
    // own scope/binder checks precede this helper and remain authoritative.
    // Neither a favorable verdict nor Unknown disables grounding ceilings.
    Ok(themelios_analysis::Analysis::of(program))
}

fn signature_sizes(signatures: impl Iterator<Item = Signature>) -> (u128, u128) {
    signatures.fold((0_u128, 0_u128), |(count, bytes), signature| {
        (
            count.saturating_add(1),
            bytes.saturating_add(signature.name.as_str().len() as u128),
        )
    })
}

#[derive(Default)]
struct PoolFree {
    nodes: u128,
    name_bytes: u128,
    pooled: bool,
}
impl Visit for PoolFree {
    fn visit_atom(&mut self, atom: &Atom) {
        self.nodes = self.nodes.saturating_add(1);
        self.name_bytes = self
            .name_bytes
            .saturating_add(atom.name.as_str().len() as u128);
        self.pooled |= !matches!(atom.arguments, Arguments::Single(_));
        for arguments in atom.alternatives() {
            for term in arguments {
                for node in term.subterms() {
                    self.visit_term(node);
                }
            }
        }
    }
    fn visit_term(&mut self, term: &Term) {
        self.nodes = self.nodes.saturating_add(1);
        self.pooled |= matches!(term, Term::Pool(_));
    }
}

#[cfg(test)]
mod tests {
    use themelios_analysis::{Analysis, Verdict};
    use themelios_base::source::{Source, SourceId};
    use themelios_program::raise::raise;
    use themelios_syntax::{dialect::Dialect, parse::parse};

    use super::*;
    use crate::{AdmissionFailure, ExpansionFailure, ExpansionLimits};

    fn input(text: &str) -> (Program, Location) {
        let source = Source::new(SourceId::new(9), text.to_owned()).expect("fixture source");
        let parsed = parse(&source, Dialect::Clingo);
        assert!(parsed.diagnostics().is_empty(), "{text}");
        let raised = raise(&parsed);
        assert!(raised.diagnostics().is_empty(), "{text}");
        (
            raised.program().clone(),
            Location {
                source: source.id(),
                span: source.span(),
            },
        )
    }

    fn run(
        text: &str,
        limits: &FormulaLimits,
        expansion: ExpansionLimits,
    ) -> Result<Analysis, FormulaFailure> {
        let (program, location) = input(text);
        analyze(
            &program,
            limits,
            &mut Budget::new(expansion, usize::MAX),
            location,
        )
    }

    #[test]
    fn raw_pools_are_refused_in_atoms_comparisons_and_aggregate_elements() {
        for text in [
            "p(1;2).",
            "p(f((1;2))).",
            "p :- (1;2)=1.",
            "p :- #count{(1;2):q}>=1.",
            "#minimize{(1;2):q}.",
        ] {
            let error = run(text, &FormulaLimits::default(), ExpansionLimits::default())
                .expect_err("pool freedom must be certified before upstream unpool");
            assert!(
                matches!(error,
                    FormulaFailure::Expansion(ExpansionFailure::Admission(
                        AdmissionFailure::Profile { feature: ProfileFeature::AnalysisPool, location }
                    )) if location.source == SourceId::new(9)
                ),
                "{text}: {error}"
            );
        }
    }

    #[test]
    fn potential_edges_include_head_conditions_and_negative_aggregate_modes() {
        for (text, edges) in [
            ("a;b :- c,d.", 4),
            ("1{a:b;c:d}1 :- e.", 6),
            ("a :- not #count{1:b}>=1.", 2),
        ] {
            let limits = FormulaLimits {
                max_analysis_edges: edges,
                ..FormulaLimits::default()
            };
            run(text, &limits, ExpansionLimits::default()).expect("inclusive edge ceiling");
            let error = run(
                text,
                &FormulaLimits {
                    max_analysis_edges: edges - 1,
                    ..limits
                },
                ExpansionLimits::default(),
            )
            .expect_err("one fewer potential edge must refuse");
            assert!(
                matches!(error, FormulaFailure::Limit {
                resource: FormulaResource::AnalysisEdges, observed, ..
            } if observed == edges as u128),
                "{text}: {error}"
            );
        }
    }

    #[test]
    fn signature_payload_and_structural_work_have_independent_inclusive_limits() {
        // Three names reserve six signature bytes; two edges reserve four
        // endpoint bytes. Four structural nodes plus two edges consume six work.
        let expansion = ExpansionLimits {
            max_term_work: 6,
            max_scalar_bytes: 10,
            ..ExpansionLimits::default()
        };
        run("a :- b,c.", &FormulaLimits::default(), expansion).expect("exact ceilings");
        for (limited, expected) in [
            (
                ExpansionLimits {
                    max_term_work: 5,
                    ..expansion
                },
                ExpansionResource::TermWork,
            ),
            (
                ExpansionLimits {
                    max_scalar_bytes: 9,
                    ..expansion
                },
                ExpansionResource::ScalarBytes,
            ),
        ] {
            let error = run("a :- b,c.", &FormulaLimits::default(), limited)
                .expect_err("preflight allowance exhausted");
            assert!(
                matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource, observed, limit, ..
            }) if resource == expected && observed > limit)
            );
        }
        let name = "a".repeat(1024);
        let source = format!("{name} :- b,c,d,e.");
        let error = run(
            &source,
            &FormulaLimits::default(),
            ExpansionLimits {
                max_scalar_bytes: 4096,
                ..ExpansionLimits::default()
            },
        )
        .expect_err("small node count cannot hide large repeated endpoint payload");
        assert!(matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource: ExpansionResource::ScalarBytes,
                ..
            })
        ));
    }

    #[test]
    fn inverse_body_growth_remains_unknown() {
        let text = "p(0). p(X) :- p(X+1).";
        let (program, _) = input(text);
        let analysis = run(text, &FormulaLimits::default(), ExpansionLimits::default())
            .expect("bounded analysis of inverse arithmetic recursion");
        assert_eq!(analysis, Analysis::of(&program));
        assert!(matches!(
            analysis.safety().finiteness(),
            Verdict::Unknown { .. }
        ));
    }

    #[test]
    fn unknown_finiteness_is_retained_without_disabling_limits() {
        let text = "p(0). p(X+1) :- p(X).";
        let (program, _) = input(text);
        let analysis = run(text, &FormulaLimits::default(), ExpansionLimits::default())
            .expect("bounded analysis of a potentially growing source");
        assert_eq!(analysis, Analysis::of(&program));
        assert!(matches!(
            analysis.safety().finiteness(),
            Verdict::Unknown { .. }
        ));
        assert!(matches!(
            run(
                text,
                &FormulaLimits {
                    max_analysis_edges: 0,
                    ..FormulaLimits::default()
                },
                ExpansionLimits::default()
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::AnalysisEdges,
                ..
            })
        ));
    }

    #[test]
    fn directives_need_an_explicit_analysis_allocation_contract() {
        let error = run(
            "#external a : b.",
            &FormulaLimits::default(),
            ExpansionLimits::default(),
        )
        .expect_err("external pseudo-rule is outside the analyzed projection");
        assert!(matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                feature: ProfileFeature::Statement,
                ..
            }))
        ));
    }
}
