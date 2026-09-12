//! Fresh candidate-dependent reduct queries over reusable allocations.

use zetesis_ferraris::{Interpretation, Theory};

use crate::{AdmissionLimits, Cnf, Incomplete, encoding, search};
use search::{Budget, Quota};

#[derive(Debug, Default)]
pub(super) struct Workspace {
    encoding: encoding::Workspace,
    search: search::Workspace,
}

impl Workspace {
    pub(super) fn reserve(
        &mut self,
        theory: &Theory,
        limits: AdmissionLimits,
    ) -> Result<(), Incomplete> {
        let dimensions = encoding::Dimensions::new(theory, limits)?;
        self.encoding.reserve(theory, limits)?;
        self.search
            .reserve(dimensions.variables, dimensions.clauses)
    }

    pub(super) fn retained_bytes(&self) -> u128 {
        self.encoding.retained_bytes() + self.search.retained_bytes()
    }

    pub(super) fn encode(
        &mut self,
        theory: &Theory,
        candidate: &Interpretation,
        limits: AdmissionLimits,
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<(&Cnf, &mut search::Workspace), Incomplete> {
        let cnf = self
            .encoding
            .encode(theory, Some(candidate), limits, budget)?;
        Ok((cnf, &mut self.search))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Check, Control, Limits, SearchStatistics, Statistics};
    use zetesis_ferraris::Node;

    fn theory(nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
        Theory::new(
            1,
            nodes,
            roots,
            zetesis_ferraris::AdmissionLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn frozen_false_nodes_do_not_keep_original_equivalences() {
        // At M={a}, F=(not a -> a) freezes to (bottom -> a). The empty
        // interpretation is a proper-subset model. Keeping n<->not a and
        // adding not n would incorrectly force a and hide that witness.
        let input = theory(
            vec![
                Node::Atom(0),
                Node::False,
                Node::Implies(0, 1),
                Node::Implies(2, 0),
            ],
            vec![3],
        );
        let fact = theory(vec![Node::Atom(0)], vec![0]);
        let mut workspace = Workspace::default();
        let control = Control::default();
        for original in [&input, &fact, &input] {
            let candidate = Interpretation::new(original, [0]).unwrap();
            let mut budget = Budget {
                quota: search::LocalQuota,
                limits: Limits::default().search,
                control: &control,
                statistics: SearchStatistics::default(),
            };
            let result = super::super::membership(
                original,
                &candidate,
                Limits::default(),
                &mut budget,
                &mut Statistics::default(),
                &mut workspace,
            )
            .unwrap();
            if original.same_instance(&fact) {
                assert!(matches!(result, Check::Stable));
            } else {
                let Check::NonMinimal(witness) = result else {
                    panic!("missing empty reduct witness: {result:?}")
                };
                assert_eq!(witness.atoms().count(), 0);
                assert!(witness.theory().same_instance(original));
                assert!(
                    zetesis_ferraris::models_reduct(
                        original,
                        &candidate,
                        &witness,
                        zetesis_ferraris::Limits::default(),
                        &control
                    )
                    .unwrap()
                );
            }
        }
    }

    #[test]
    fn reservation_failure_does_not_poison_later_encoding() {
        let huge = Theory::new(
            usize::MAX / 2,
            vec![],
            vec![],
            zetesis_ferraris::AdmissionLimits {
                max_atoms: usize::MAX,
                ..Default::default()
            },
        )
        .unwrap();
        let mut workspace = Workspace::default();
        let limits = AdmissionLimits {
            max_variables: usize::MAX,
            max_clauses: 0,
            max_literals: 0,
        };
        assert_eq!(
            workspace.reserve(&huge, limits),
            Err(Incomplete::Allocation)
        );
        let fact = theory(vec![Node::Atom(0)], vec![0]);
        let candidate = Interpretation::new(&fact, [0]).unwrap();
        let control = Control::default();
        let mut budget = Budget {
            quota: search::LocalQuota,
            limits: Limits::default().search,
            control: &control,
            statistics: SearchStatistics::default(),
        };
        let (cnf, search) = workspace
            .encode(&fact, &candidate, AdmissionLimits::default(), &mut budget)
            .unwrap();
        assert!(matches!(
            search.query(cnf, &mut budget),
            crate::Solve::Unsat
        ));
    }

    #[test]
    fn batch_completion_releases_the_previous_scalar_workspace() {
        let choice = theory(
            vec![
                Node::Atom(0),
                Node::False,
                Node::Implies(0, 1),
                Node::Or(0, 2),
            ],
            vec![3],
        );
        let mut models =
            crate::StableModels::new(&choice, Limits::default(), Control::default()).unwrap();
        let first = models.next().unwrap().unwrap();
        let empty = Workspace::default().retained_bytes();
        assert!(models.reduct_workspace.retained_bytes() > empty);
        let next = models
            .next_batch(
                crate::BatchLimits {
                    max_candidates: std::num::NonZeroUsize::new(2).unwrap(),
                    max_pending_bytes: 1024,
                },
                |_, candidates| {
                    Ok::<_, std::convert::Infallible>(vec![
                        crate::BatchVerdict::Residual;
                        candidates.len()
                    ])
                },
            )
            .unwrap();
        assert_eq!(models.reduct_workspace.retained_bytes(), empty);
        assert_eq!(next.len(), 1);
        assert_ne!(
            first.atoms().collect::<Vec<_>>(),
            next[0].atoms().collect::<Vec<_>>()
        );
        assert_eq!(models.statistics().stable_models, 2);
    }
}
