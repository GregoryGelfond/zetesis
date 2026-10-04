//! Original ownership through canonical collection of analyzed statements.
//!
//! Compilation appends an already-canonical family for each original statement.
//! A family of length other than one is ineligible for an in-place keyed rewrite.
//! Canonical collection can also merge outputs from different owners, or repeated
//! outputs from one owner: every content collision is likewise ineligible.
//!
//! The sidecar retains only statement IDs. Its temporary map borrows content;
//! neither statement payloads nor provenance are copied. Map iteration gives the
//! canonical statement order consumed by `Program::of_nodes`. This relies on the
//! private producer invariant: normalization/projection outputs have passed the
//! upstream Program door, and expanded facts contain only already-canonical
//! symbolic arguments; normalized weak objectives rewrap canonical fields.
//! It is not a constructor over arbitrary raw statements.
//!
//! For m emitted carriers this uses O(m log m) content comparisons and O(m)
//! identity/map scratch. Each comparison reads bounded statement structure.
//! The term-work unit charged here is one carrier lookup, not each node visited
//! by upstream Ord; byte charges cover named sidecar/map payload, not allocator
//! bookkeeping. Canonical Program construction retains its upstream cost.

use std::collections::BTreeMap;

use themelios_program::program::{Program, Statement};
use themelios_program::provenance::WithProvenance;

use crate::expansion::Budget;
use crate::{ExpansionResource, FormulaFailure, ProgramSite, StatementId};

/// One entry per canonical analyzed carrier, in its content order. An absent
/// owner means expansion or content merging prevents an in-place rewrite.
pub(crate) struct Owners {
    entries: Vec<Option<StatementId>>,
}

/// Record the analyzed family just emitted by one original statement. Every
/// original is recorded once; a repeated count of one must not be used to hide
/// multiple emissions from the same source compilation.
pub(crate) fn record(
    owners: &mut Vec<Option<StatementId>>,
    owner: StatementId,
    count: usize,
    budget: &mut Budget,
    site: ProgramSite,
) -> Result<(), FormulaFailure> {
    crate::formula_pattern::reserve(owners, count, budget, site)?;
    let identity = (count == 1).then_some(owner);
    owners.extend(std::iter::repeat_n(identity, count));
    Ok(())
}

impl Owners {
    /// Collect these already-canonical carriers and their parallel ownership
    /// vector together, so the sidecar cannot be paired with another collection.
    /// Two equal carriers clear ownership even if their original ID agrees.
    pub(crate) fn collect(
        statements: Vec<WithProvenance<Statement>>,
        owners: Vec<Option<StatementId>>,
        budget: &mut Budget,
        site: ProgramSite,
    ) -> Result<(Program, Self), FormulaFailure> {
        assert_eq!(
            statements.len(),
            owners.len(),
            "every analyzed carrier has an owner slot"
        );
        let mut content = BTreeMap::new();
        for (statement, owner) in statements.iter().zip(&owners) {
            budget.charge(ExpansionResource::TermWork, 1, site)?;
            // Named key/value payload; BTreeMap allocator overhead is outside
            // the expansion allowance, as for the neighboring compiler maps.
            budget.charge(
                ExpansionResource::ScalarBytes,
                size_of::<(&Statement, Option<StatementId>)>() as u128,
                site,
            )?;
            content
                .entry(statement.get())
                .and_modify(|owner| *owner = None)
                .or_insert(*owner);
        }
        let mut entries = Vec::new();
        crate::formula_pattern::reserve(&mut entries, content.len(), budget, site)?;
        entries.extend(content.into_values());
        drop(owners);
        let program = Program::of_nodes(statements);
        assert_eq!(
            program.statements().count(),
            entries.len(),
            "canonical analyzed collection preserves sidecar positions",
        );
        Ok((program, Self { entries }))
    }

    /// An owner may be asked only for this canonical analyzed position.
    pub(crate) fn at(&self, index: usize) -> Option<StatementId> {
        self.entries
            .get(index)
            .copied()
            .expect("the owner sidecar covers every canonical analyzed position")
    }
}

#[cfg(test)]
mod tests {
    use super::{Owners, record};
    use crate::expansion::{Budget, ExpansionLimits};
    use crate::{ProgramSite, StatementId};
    use themelios_program::program::{Atom, Program, Rule, Statement};
    use themelios_program::provenance::{Origin, Provenance, TransformTag, WithProvenance};
    use themelios_program::symbol::Name;

    fn fact(name: &str) -> WithProvenance<Statement> {
        Program::of([Rule::fact(Atom::constant(Name::new(name).unwrap()))])
            .statements()
            .next()
            .unwrap()
            .clone()
    }

    fn collect(families: Vec<(usize, Vec<WithProvenance<Statement>>)>) -> (Program, Owners) {
        let site = ProgramSite::program();
        let mut budget = Budget::new(ExpansionLimits::default(), 100);
        let mut statements = Vec::new();
        let mut owners = Vec::new();
        for (owner, family) in families {
            record(
                &mut owners,
                StatementId::new(owner),
                family.len(),
                &mut budget,
                site,
            )
            .unwrap();
            statements.extend(family);
        }
        Owners::collect(statements, owners, &mut budget, site).unwrap()
    }

    #[test]
    fn constructed_owners_follow_canonical_content_order() {
        let (program, owners) = collect(vec![(7, vec![fact("z")]), (3, vec![fact("a")])]);
        assert_eq!(program.statements().count(), 2);
        assert_eq!(owners.at(0), Some(StatementId::new(3)));
        assert_eq!(owners.at(1), Some(StatementId::new(7)));
    }

    #[test]
    fn distinct_owners_of_equal_content_are_ineligible() {
        let (program, owners) = collect(vec![(1, vec![fact("p")]), (2, vec![fact("p")])]);
        assert_eq!(program.statements().count(), 1);
        assert_eq!(owners.at(0), None);
    }

    #[test]
    fn expanded_owners_remain_ineligible() {
        for family in [vec![fact("p"), fact("q")], vec![fact("p"), fact("p")]] {
            let (program, owners) = collect(vec![(1, family)]);
            for index in 0..program.statements().count() {
                assert_eq!(owners.at(index), None);
            }
        }
    }

    fn compiled(text: &str, max_key_work: u64) -> crate::AdmittedFormula {
        let parsed =
            crate::ParsedSource::new(text.into(), crate::AdmissionOptions::default()).unwrap();
        let raised = themelios_program::raise::raise(parsed.parsed());
        assert!(raised.diagnostics().is_empty());
        // The outer statements are constructed, so no parsed coordinate can
        // supply the keyed-rewrite identity exercised here.
        let program = Program::of(
            raised
                .into_program()
                .statements()
                .map(|carrier| carrier.get().clone()),
        );
        crate::prepare_program_formula(
            std::sync::Arc::new(program),
            crate::ProgramAdmissionOptions::default(),
            ExpansionLimits::default(),
            crate::FormulaLimits {
                max_key_work,
                ..crate::FormulaLimits::default()
            },
        )
        .unwrap()
        .ground()
        .unwrap()
    }

    fn family(
        input: &crate::AdmittedFormula,
    ) -> std::collections::BTreeSet<Vec<themelios_program::Symbol>> {
        let mut search = zetesis_sat::StableModels::new(
            input.theory(),
            zetesis_sat::Limits::default(),
            zetesis_cpu::Cancellation::default(),
        )
        .unwrap();
        let result = search
            .by_ref()
            .map(|model| {
                let mut atoms: Vec<_> = model
                    .unwrap()
                    .atoms()
                    .map(|index| {
                        crate::symbols::atom_with(input.atoms().at(index).unwrap(), 4096, || {
                            Ok::<_, std::convert::Infallible>(())
                        })
                        .unwrap()
                    })
                    .collect();
                atoms.sort_unstable();
                atoms
            })
            .collect();
        assert!(search.exhausted());
        result
    }

    #[test]
    fn constant_normalization_keeps_a_unique_compiler_owner() {
        let source = "#const target=a. key(a). value(0;1). \
            1{p(K,V):value(V)}1 :- key(K). :- p(target,V), V != 1.";
        let rewritten = compiled(source, crate::FormulaLimits::default().max_key_work);
        let original = compiled(source, 0);
        assert_eq!(rewritten.keyed_constraints(), 1);
        assert_eq!(original.keyed_constraints(), 0);
        let actual = family(&rewritten);
        assert_eq!(actual.len(), 1);
        assert_eq!(actual, family(&original));
    }

    #[test]
    fn expanded_compiler_collisions_remain_original() {
        let source = "#const selected=1. key(a;b). value(0;1). \
            1{p(K,V):value(V)}1 :- key(K). \
            :- p((a;b),V), V != selected. :- p(a,V), V != 1.";
        let inspected = compiled(source, crate::FormulaLimits::default().max_key_work);
        let original = compiled(source, 0);
        assert_eq!(inspected.keyed_constraints(), 0);
        let actual = family(&inspected);
        assert_eq!(actual.len(), 1);
        assert_eq!(actual, family(&original));
    }

    #[test]
    fn provenance_does_not_determine_ownership() {
        let provenance = Provenance::from(Origin::Transformed(TransformTag::new("shared")));
        let (program, owners) = collect(vec![
            (
                1,
                vec![WithProvenance::new(
                    fact("p").into_value(),
                    provenance.clone(),
                )],
            ),
            (
                2,
                vec![WithProvenance::new(fact("q").into_value(), provenance)],
            ),
        ]);
        assert_eq!(program.statements().count(), 2);
        assert_eq!(owners.at(0), Some(StatementId::new(1)));
        assert_eq!(owners.at(1), Some(StatementId::new(2)));
    }
}
