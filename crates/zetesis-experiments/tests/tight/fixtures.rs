use super::*;
use std::num::NonZeroUsize;
use zetesis_cpu::Control;
use zetesis_ferraris::{TightPlan, TightPlanLimits};

fn fixture(family: Family, atoms: usize) -> Fixture {
    build(Case {
        family,
        atoms: NonZeroUsize::new(atoms).unwrap(),
        candidates: NonZeroUsize::new(4).unwrap(),
        reference: super::super::Reference::GeneralReduct,
    })
    .unwrap()
}

#[test]
fn support_distributions_preserve_the_choice_dag() {
    for atoms in [2, 64, 2048, 4096] {
        let original = fixture(Family::Choices, atoms);
        for family in [Family::SupportUniform, Family::SupportSkewed] {
            let tested = fixture(family, atoms);
            assert_eq!(tested.theory.nodes(), original.theory.nodes());
            assert_eq!(&tested.theory.roots()[..atoms - 1], original.theory.roots());
            assert_eq!(
                tested
                    .theory
                    .roots()
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>(),
                original
                    .theory
                    .roots()
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
            );
            assert_eq!(tested.theory.roots().len(), 16 * (atoms - 1));
        }
    }
}

#[test]
fn support_distributions_have_the_declared_producer_counts() {
    for atoms in [2, 33, 65, 4096] {
        for family in [Family::SupportUniform, Family::SupportSkewed] {
            let fixture = fixture(family, atoms);
            let plan = TightPlan::compile(
                &fixture.theory,
                TightPlanLimits::default(),
                &Control::default(),
            )
            .unwrap();
            let mut counts = vec![0usize; atoms];
            for producer in plan.producers() {
                counts[producer.head()] += 1;
            }
            let expected: Vec<_> = (0..atoms)
                .map(|atom| {
                    if atom == atoms - 1 {
                        0
                    } else if family == Family::SupportUniform {
                        16
                    } else if atom == atoms - 2 {
                        1 + 15 * (atoms - 1)
                    } else {
                        1
                    }
                })
                .collect();
            assert_eq!(counts, expected);
        }
    }
}

#[test]
fn support_distributions_keep_candidate_occurrence_order() {
    let original = fixture(Family::Choices, 65);
    for family in [Family::SupportUniform, Family::SupportSkewed] {
        let tested = fixture(family, 65);
        let atoms = |fixture: &Fixture| {
            fixture
                .candidates
                .iter()
                .map(|candidate| candidate.atoms().collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(atoms(&tested), atoms(&original));
    }
}
