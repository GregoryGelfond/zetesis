//! The deferred owner moves real source identity and accepted accounting.

use super::PreparedFormula;
use crate::formula_support::{Computation, Counters, Support};
use crate::grounding_observer::Work;
use crate::{
    AdmissionOptions, ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure,
    FormulaLimits, FormulaResource, PreparedFormulaBundle, prepare_formula,
};
use zetesis_core::{Value, ValueNodeRef, catalog::TermKey};

const SOURCE: &str = "p(1). q(X) :- p(X). :- q(2).";
const RETAINED: &str = "identity admitted before materialization";

fn prepared_with_identity() -> (PreparedFormula, TermKey) {
    let mut prepared = prepare_formula(
        SOURCE.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let preparation = &mut prepared.preparation;
    let mut counters =
        Counters::resume(std::mem::take(&mut preparation.accounting), Work::default());
    let location = preparation.location;
    let key = {
        let (relations, mut append) = preparation
            .catalog
            .split(&preparation.limits, &mut counters, location)
            .unwrap();
        let support =
            Support::indexed(&relations, &preparation.limits, &counters, location).unwrap();
        let mut computation = Computation::new(&mut append, &support);
        let value = Value::String(RETAINED.into());
        let key = computation
            .import(
                (&value).into(),
                &preparation.limits,
                &mut counters,
                location,
            )
            .unwrap();
        counters
            .generated(&key, &computation, &preparation.limits, location)
            .unwrap();
        key
    };
    preparation.accounting = counters.into_accounting();
    assert!(preparation.accounting.work > 0);
    (prepared, key)
}

#[test]
fn moved_preparation_preserves_source_identity_through_both_public_doors() {
    fn requires_send<T: Send>() {}
    requires_send::<PreparedFormula>();
    requires_send::<PreparedFormulaBundle>();
    for hybrid in [false, true] {
        let (prepared, key) = prepared_with_identity();
        // The owner, its generated history and the original scoped key cross the
        // boundary together; the final reader must accept that original key.
        std::thread::spawn(move || {
            let original = std::ptr::from_ref(prepared.original_program());
            let catalog = if hybrid {
                let result = prepared.ground_hybrid().unwrap();
                assert_eq!(result.source().unwrap().text(), SOURCE);
                assert_eq!(std::ptr::from_ref(result.original_program()), original);
                result.atom_catalog().clone()
            } else {
                let result = prepared.ground().unwrap();
                assert_eq!(result.source().expect("source input").text(), SOURCE);
                assert_eq!(std::ptr::from_ref(result.original_program()), original);
                result.atom_catalog().clone()
            };
            assert_eq!(
                catalog.read().term(&key).unwrap().descriptor(),
                ValueNodeRef::String(RETAINED)
            );
            assert!(
                catalog
                    .atoms()
                    .iter()
                    .any(|atom| atom.predicate().name() == "p")
            );
            assert!(
                catalog
                    .atoms()
                    .iter()
                    .any(|atom| atom.predicate().name() == "q")
            );
        })
        .join()
        .unwrap();
    }
}

#[test]
fn grounding_cannot_reset_work_accepted_before_the_handoff() {
    for hybrid in [false, true] {
        let (mut prepared, _key) = prepared_with_identity();
        // Seal this private fixture's allowance exactly at its real admission
        // receipt. Both public materialization doors must refuse their next tick.
        let accepted = prepared.preparation.accounting.work;
        prepared.preparation.limits.max_work = accepted;
        let error = if hybrid {
            prepared.ground_hybrid().unwrap_err()
        } else {
            prepared.ground().unwrap_err()
        };
        assert!(matches!(error, FormulaFailure::Limit {
            resource: FormulaResource::Work, limit, observed, ..
        } if limit == u128::from(accepted) && observed == u128::from(accepted) + 1));
    }
}

#[test]
fn grounding_retains_an_independent_scalar_budget_charge() {
    let source = "p(f(\"x\")). q(X) :- p(f(X)).";
    for hybrid in [false, true] {
        let prepare = |expansion| {
            prepare_formula(
                source.into(),
                AdmissionOptions::default(),
                expansion,
                FormulaLimits::default(),
            )
            .unwrap()
        };
        let finish = |prepared: PreparedFormula| {
            if hybrid {
                prepared
                    .ground_hybrid()
                    .map(|result| *result.expansion_usage())
            } else {
                prepared.ground().map(|result| *result.expansion_usage())
            }
        };
        let prepared = prepare(ExpansionLimits::default());
        let accepted = prepared.preparation.budget.usage().scalar_bytes;
        let baseline = finish(prepared).unwrap();
        assert!(accepted > 0);
        assert!(baseline.scalar_bytes > accepted);

        // This accepted byte is independent of the materialization path that
        // supplies the baseline. Resetting both paths cannot erase the delta.
        let charge = |prepared: &mut PreparedFormula| {
            let preparation = &mut prepared.preparation;
            assert_eq!(preparation.budget.usage().scalar_bytes, accepted);
            preparation
                .budget
                .charge(ExpansionResource::ScalarBytes, 1, preparation.location)
                .unwrap();
            assert_eq!(preparation.budget.usage().scalar_bytes, accepted + 1);
        };
        let mut prepared = prepare(ExpansionLimits::default());
        charge(&mut prepared);
        let mut expected = baseline;
        expected.scalar_bytes += 1;
        assert_eq!(finish(prepared).unwrap(), expected);

        // The same retained byte also crosses the exact successful ceiling.
        let mut prepared = prepare(ExpansionLimits {
            max_scalar_bytes: baseline.scalar_bytes,
            ..ExpansionLimits::default()
        });
        charge(&mut prepared);
        assert!(matches!(
            finish(prepared),
            Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource: ExpansionResource::ScalarBytes, limit, observed, ..
            })) if limit == baseline.scalar_bytes as u128
                && observed == baseline.scalar_bytes as u128 + 1
        ));
    }
}

#[test]
fn prepared_einstein_retains_a_terminal_flat_definition() {
    let prepared = prepare_formula(
        include_str!("../../../../../examples/einstein-riddle.lp").into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepared.analysis_basis(),
        crate::AnalysisBasis::NormalizedProgram
    );
    let analysis = zetesis_domain::terminal::analyze(
        prepared.analyzed_program(),
        crate::DomainLimits::default(),
    );
    assert_eq!(
        analysis.status(),
        zetesis_domain::terminal::Status::Complete
    );
    assert!(analysis.belongs_to(prepared.analyzed_program()));
    assert_eq!(analysis.definitions().len(), 1);
    let themelios_program::program::Statement::Rule(source_rule) = analysis.definitions()[0].get()
    else {
        panic!("a terminal definition is a rule");
    };
    let themelios_program::program::Head::Literal(literal) = source_rule.head().get() else {
        panic!("a terminal definition has one ordinary head");
    };
    let themelios_program::program::LiteralInner::Atom(atom) = &literal.inner else {
        panic!("a terminal head is an atom");
    };
    assert_eq!(atom.get().name.as_str(), "solution");
    assert_eq!(source_rule.body().get().elements().count(), 5);
    // This establishes the prepared fixture's profile, not the future complete
    // source-to-IR correspondence or answer-reconstruction equivalence.
    let preparation = &prepared.preparation;
    let mut counters = Counters::default();
    let components = preparation
        .catalog
        .component_view(&preparation.limits, &mut counters, preparation.location)
        .unwrap()
        .unwrap();
    let rules: Vec<_> = preparation
        .program
        .rules
        .iter()
        .filter(|rule| {
            let crate::formula_ir::HeadIr::Normal(Some(head)) = rule.head else {
                return false;
            };
            head.get(
                components,
                &preparation.limits,
                &mut counters,
                rule.location,
            )
            .unwrap()
            .predicate()
            .name()
                == "solution"
        })
        .collect();
    assert_eq!(rules.len(), 1);
    let rule = rules[0];
    assert_eq!(rule.variables, 6);
    assert_eq!(rule.body_variables, rule.variables);
    assert!(rule.bindings.is_none());
    assert_eq!(rule.body.len(), 5);
    assert!(rule.body.iter().all(|literal| matches!(
        literal,
        crate::formula_ir::LiteralIr::Atom(themelios_program::program::DefaultNegation::None, _)
    )));
}
