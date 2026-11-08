//! Original and coherence roots identified by their public emission phases.

use std::cell::RefCell;
use zetesis_ferraris::{NodeView, Theory};
use zetesis_themelios::{
    AdmittedFormula, AdmittedFormulaBundle, GroundingObserver, GroundingOutcome, GroundingPhase,
    GroundingWork, PreparedFormula, PreparedFormulaBundle, ProgramSite,
};

#[derive(Default)]
pub(crate) struct Roots(RefCell<Vec<(GroundingPhase, GroundingOutcome, Option<u64>)>>);

impl GroundingObserver for Roots {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn domain_diagnostics_enabled(&self) -> bool {
        false
    }
    fn phase_exit(
        &self,
        phase: GroundingPhase,
        _site: Option<ProgramSite>,
        outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.borrow_mut().push((phase, outcome, work.roots));
    }
}

impl Roots {
    pub(crate) fn indices(&self, selected: GroundingPhase) -> Vec<usize> {
        let mut offset = 0_usize;
        let mut indices = Vec::new();
        for &(phase, outcome, roots) in self.0.borrow().iter() {
            assert_eq!(outcome, GroundingOutcome::Completed);
            let count = usize::try_from(roots.expect("bounded root count")).unwrap();
            let end = offset.checked_add(count).unwrap();
            if phase == selected {
                indices.extend(offset..end);
            }
            offset = end;
        }
        indices
    }

    fn check_total(&self, theory: &Theory) {
        let total = self
            .0
            .borrow()
            .iter()
            .map(|(_, _, count)| usize::try_from(count.expect("bounded root count")).unwrap())
            .sum::<usize>();
        assert_eq!(
            total,
            theory.roots().len(),
            "all emitted roots have a phase"
        );
    }
}

pub(crate) fn ground(prepared: PreparedFormula) -> (AdmittedFormula, Roots) {
    let roots = Roots::default();
    let input = prepared.ground_with_observer(Some(&roots)).unwrap();
    roots.check_total(input.theory());
    (input, roots)
}

pub(crate) fn bundle(prepared: PreparedFormulaBundle) -> (AdmittedFormulaBundle, Roots) {
    let roots = Roots::default();
    let input = prepared.ground_with_observer(Some(&roots)).unwrap();
    roots.check_total(input.theory());
    (input, roots)
}

pub(crate) fn source(text: &str) -> (AdmittedFormula, Roots) {
    ground(zetesis_reference_support::prepare(text).unwrap())
}

/// A fact root is top implies its atom; support guards have a different shape.
pub(crate) fn fact_atom(theory: &Theory, root: usize) -> usize {
    let view = theory.view();
    let NodeView::Implies(truth, atom) = view.node(root).unwrap() else {
        panic!("fact implication");
    };
    let NodeView::Implies(left, right) = view.node(truth).unwrap() else {
        panic!("truth antecedent");
    };
    assert_eq!(view.node(left).unwrap(), NodeView::False);
    assert_eq!(view.node(right).unwrap(), NodeView::False);
    let NodeView::Atom(atom) = view.node(atom).unwrap() else {
        panic!("fact atom");
    };
    atom
}

/// Coherence excludes the conjunction of two concrete atom identities.
pub(crate) fn coherence_atoms(theory: &Theory, root: usize) -> [usize; 2] {
    let view = theory.view();
    let NodeView::Implies(body, consequence) = view.node(root).unwrap() else {
        panic!("coherence implication");
    };
    assert_eq!(view.node(consequence).unwrap(), NodeView::False);
    let NodeView::And(operands) = view.node(body).unwrap() else {
        panic!("opposite atoms conjoined");
    };
    let [first, second] = operands else {
        panic!("exactly two opposite atoms");
    };
    [first, second].map(|&node| {
        let NodeView::Atom(atom) = view.node(node).unwrap() else {
            panic!("concrete atom identity");
        };
        atom
    })
}
