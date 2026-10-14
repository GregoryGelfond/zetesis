//! Generated tight theories and candidate sequences: a workspace that updates
//! its retained evaluation returns exactly the verdict of a complete one.

use proptest::prelude::*;

use super::TightWorkspace;
use crate::{
    AdmissionError, AdmissionLimits, ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts,
    FormulaTransaction, Interpretation, Node, NodeView, Theory, TightCheckLimits, TightPlan,
    TightPlanLimits,
};
use zetesis_cpu::Cancellation;

const ATOMS: usize = 6;

#[derive(Debug, Clone)]
enum Body {
    Group(bool, Vec<usize>),
    Not(usize),
    Threshold(usize, usize, u64, u64, i128),
    /// Two conditions with ranks in a four-rank carrier, cut at a prefix or a
    /// suffix boundary.
    Extremum(usize, usize, u64, u64, bool, u64),
}

#[derive(Debug, Clone)]
enum Rule {
    Normal(usize, Option<usize>),
    Choice(usize, Option<usize>),
    Constraint(usize),
}

#[derive(Debug, Clone)]
struct Case {
    bodies: Vec<Body>,
    rules: Vec<Rule>,
    candidates: Vec<(u8, Option<u64>)>,
}

fn body() -> impl Strategy<Value = Body> {
    prop_oneof![
        (any::<bool>(), prop::collection::vec(any::<usize>(), 2..4))
            .prop_map(|(or, picks)| Body::Group(or, picks)),
        any::<usize>().prop_map(Body::Not),
        (
            any::<usize>(),
            any::<usize>(),
            0..3_u64,
            0..3_u64,
            0..4_i128
        )
            .prop_map(|(a, b, x, y, bound)| Body::Threshold(a, b, x, y, bound)),
        (
            any::<usize>(),
            any::<usize>(),
            0..4_u64,
            0..4_u64,
            any::<bool>(),
            0..5_u64
        )
            .prop_map(|(a, b, x, y, prefix, cut)| Body::Extremum(a, b, x, y, prefix, cut)),
    ]
}

fn rule() -> impl Strategy<Value = Rule> {
    prop_oneof![
        (any::<usize>(), prop::option::of(any::<usize>()))
            .prop_map(|(head, body)| Rule::Normal(head, body)),
        (any::<usize>(), prop::option::of(any::<usize>()))
            .prop_map(|(head, body)| Rule::Choice(head, body)),
        any::<usize>().prop_map(Rule::Constraint),
    ]
}

fn case() -> impl Strategy<Value = Case> {
    (
        prop::collection::vec(body(), 1..10),
        prop::collection::vec(rule(), 1..10),
        prop::collection::vec((any::<u8>(), prop::option::of(0..40_u64)), 2..16),
    )
        .prop_map(|(bodies, rules, candidates)| Case {
            bodies,
            rules,
            candidates,
        })
}

impl Body {
    /// Append this body over the nodes built so far; `None` when its picks
    /// name too few distinct conditions, and an error when admission refuses.
    fn push(
        &self,
        append: &mut FormulaTransaction<'_>,
        built: &[usize],
        falsum: usize,
    ) -> Result<Option<usize>, AdmissionError> {
        let pick = |index: &usize| built[index % built.len()];
        Ok(Some(match self {
            Self::Group(or, picks) => {
                let mut operands: Vec<usize> = picks.iter().map(pick).collect();
                operands.sort_unstable();
                operands.dedup();
                let view = if *or {
                    NodeView::Or(&operands)
                } else {
                    NodeView::And(&operands)
                };
                append.push(view, usize::MAX, usize::MAX)?
            }
            Self::Not(index) => append.push(
                NodeView::Implies(pick(index), falsum),
                usize::MAX,
                usize::MAX,
            )?,
            Self::Threshold(first, second, x, y, bound) => {
                let mut conditions = [pick(first), pick(second)];
                conditions.sort_unstable();
                if conditions[0] == conditions[1] {
                    return Ok(None);
                }
                append.push_threshold_family(
                    &conditions,
                    &[*x, *y],
                    &[*bound],
                    usize::MAX,
                    usize::MAX,
                )?[0]
            }
            Self::Extremum(first, second, x, y, prefix, cut) => {
                let rule = if *prefix {
                    ExtremumCutRule::Prefix(*cut)
                } else {
                    ExtremumCutRule::Suffix(*cut)
                };
                append
                    .push_extremum_cut_family_range(
                        ExtremumRow {
                            conditions: &[pick(first), pick(second)],
                            ranks: &[*x, *y],
                            rank_count: 4,
                        },
                        &[rule],
                        usize::MAX,
                        usize::MAX,
                    )?
                    .start
            }
        }))
    }
}

impl Rule {
    /// Append this rule's root over the bodies built so far.
    fn push(
        &self,
        append: &mut FormulaTransaction<'_>,
        built: &[usize],
        falsum: usize,
    ) -> Result<usize, AdmissionError> {
        let pick = |index: usize| built[index % built.len()];
        match *self {
            Self::Normal(head, None) => Ok(head % ATOMS),
            Self::Normal(head, Some(body)) => append.push(
                NodeView::Implies(pick(body), head % ATOMS),
                usize::MAX,
                usize::MAX,
            ),
            Self::Choice(head, body) => {
                let head = head % ATOMS;
                let not = append.push(NodeView::Implies(head, falsum), usize::MAX, usize::MAX)?;
                let choice = append.push(NodeView::Or(&[head, not]), usize::MAX, usize::MAX)?;
                match body {
                    None => Ok(choice),
                    Some(body) => append.push(
                        NodeView::Implies(pick(body), choice),
                        usize::MAX,
                        usize::MAX,
                    ),
                }
            }
            Self::Constraint(body) => append.push(
                NodeView::Implies(pick(body), falsum),
                usize::MAX,
                usize::MAX,
            ),
        }
    }
}

impl Case {
    /// The theory, or none when admission refuses a generated shape.
    fn theory(&self) -> Option<Theory> {
        let mut nodes = FormulaNodes::new(
            FormulaParts::new((0..ATOMS).map(Node::atom).collect(), vec![]).unwrap(),
        );
        let mut built: Vec<usize> = (0..ATOMS).collect();
        let mut append = nodes.transaction();
        let falsum = append.push(NodeView::False, usize::MAX, usize::MAX).ok()?;
        for body in &self.bodies {
            if let Some(node) = body.push(&mut append, &built, falsum).ok()? {
                built.push(node);
            }
        }
        let mut roots = Vec::new();
        for rule in &self.rules {
            roots.push(rule.push(&mut append, &built, falsum).ok()?);
        }
        append.commit();
        Theory::new(ATOMS, nodes.into_parts(), roots, AdmissionLimits::default()).ok()
    }

    /// The theory and its plan with the readers index, or none when admission
    /// refuses the shape or the theory has no tight plan (a positive cycle or
    /// an unsupported root).
    fn prepared(&self) -> Option<(Theory, TightPlan)> {
        let theory = self.theory()?;
        let cancellation = Cancellation::default();
        let mut plan =
            TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).ok()?;
        let prepared = plan.prepare_updates(TightPlanLimits::default(), &cancellation);
        assert_eq!(prepared.result, Ok(true));
        Some((theory, plan))
    }

    /// Each generated candidate, with the work limit of an interrupted check
    /// to run before it, when there is one.
    fn candidates<'a>(
        &'a self,
        theory: &'a Theory,
    ) -> impl Iterator<Item = (Interpretation, Option<u64>)> + 'a {
        self.candidates.iter().map(|&(mask, interrupt)| {
            let atoms = (0..ATOMS).filter(move |&atom| mask & (1 << atom) != 0);
            (Interpretation::new(theory, atoms).unwrap(), interrupt)
        })
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Every check of a reused workspace, including checks that follow an
    /// interrupted one, returns the verdict of a complete evaluation of the
    /// same candidate.
    #[test]
    fn updated_verdicts_equal_complete_verdicts(case in case()) {
        let prepared = case.prepared();
        prop_assume!(prepared.is_some());
        let (theory, plan) = prepared.unwrap();
        let cancellation = Cancellation::default();
        let mut workspace = TightWorkspace::default();
        for (candidate, interrupt) in case.candidates(&theory) {
            let complete = plan
                .check_accounted(&candidate, TightCheckLimits::default(), &cancellation)
                .result
                .unwrap()
                .verdict;
            if let Some(max_work) = interrupt {
                let limits = TightCheckLimits { max_work, ..TightCheckLimits::default() };
                if let Ok(check) = workspace.check(&plan, &candidate, limits, &cancellation).result {
                    prop_assert_eq!(check.verdict, complete);
                }
            }
            let updated = workspace
                .check(&plan, &candidate, TightCheckLimits::default(), &cancellation)
                .result
                .unwrap()
                .verdict;
            prop_assert_eq!(updated, complete, "candidate {:?}", candidate);
        }
    }

    /// Every check of a reused workspace charges at most its route's term of
    /// the plan's per-check bound: an update when the workspace kept the
    /// previous check's evaluation, otherwise a complete evaluation.
    #[test]
    fn every_check_fits_its_route_bound(case in case()) {
        let prepared = case.prepared();
        prop_assume!(prepared.is_some());
        let (theory, plan) = prepared.unwrap();
        let cancellation = Cancellation::default();
        let mut workspace = TightWorkspace::default();
        // Six atoms never reach the large-change threshold of 64, so a kept
        // evaluation is always updated.
        let mut kept = false;
        for (candidate, interrupt) in case.candidates(&theory) {
            if let Some(max_work) = interrupt {
                let limits = TightCheckLimits { max_work, ..TightCheckLimits::default() };
                kept = workspace.check(&plan, &candidate, limits, &cancellation).result.is_ok();
            }
            let check = workspace.check(&plan, &candidate, TightCheckLimits::default(), &cancellation);
            prop_assert!(check.result.is_ok());
            let bound = if kept {
                plan.update_work_bound().unwrap()
            } else {
                plan.complete_work_bound()
            };
            prop_assert!(u128::from(check.work) <= bound, "{} against {}", check.work, bound);
            kept = true;
        }
    }
}
