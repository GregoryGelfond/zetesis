//! The builder's Boolean identities and failed-initialization accounting.

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};

use super::*;
use crate::ExpansionLimits;
use crate::formula_source_activity::Context;
use crate::formula_support::testing::Fixture;

fn location() -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(17),
        span: Span::empty(ByteOffset::new(3)),
    })
}

fn with_builder<T>(
    limits: &FormulaLimits,
    purpose: Purpose,
    run: impl FnOnce(&mut Builder<'_, '_, '_>) -> T,
) -> T {
    Fixture::default().with(location(), |_, computation, counters| {
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut builder = Builder::empty(
            computation,
            limits,
            &mut budget,
            counters,
            purpose,
            None,
            location(),
        )
        .unwrap();
        run(&mut builder)
    })
}

fn constants(root: usize) -> Theory {
    let limits = FormulaLimits::default();
    with_builder(&limits, Purpose::Theory, |builder| {
        builder.initialize(location()).unwrap();
        Theory::new(
            1,
            std::mem::take(&mut builder.nodes).into_parts(),
            vec![root],
            limits.theory,
        )
        .unwrap()
    })
}

#[test]
fn canonical_constants_obey_original_truth() {
    for truth in [false, true] {
        let theory = constants(boolean(truth));
        for atoms in [vec![], vec![0]] {
            let interpretation = Interpretation::new(&theory, atoms).unwrap();
            assert_eq!(
                models(
                    &theory,
                    &interpretation,
                    Limits::default(),
                    &Cancellation::default()
                )
                .unwrap(),
                truth
            );
        }
    }
}

#[test]
fn canonical_constants_obey_frozen_truth() {
    for truth in [false, true] {
        let theory = constants(boolean(truth));
        for candidate in [vec![], vec![0]] {
            let candidate = Interpretation::new(&theory, candidate).unwrap();
            for tested in [vec![], vec![0]] {
                let tested = Interpretation::new(&theory, tested).unwrap();
                assert_eq!(
                    models_reduct(
                        &theory,
                        &candidate,
                        &tested,
                        Limits::default(),
                        &Cancellation::default()
                    )
                    .unwrap(),
                    truth
                );
            }
        }
    }
}

#[test]
fn initialization_obeys_node_ceiling() {
    for purpose in [Purpose::Theory, Purpose::Objective, Purpose::Validation] {
        for cap in 0..=2 {
            let mut limits = FormulaLimits::default();
            let resource = match purpose {
                Purpose::Theory | Purpose::Validation => {
                    limits.theory.max_nodes = cap;
                    FormulaResource::Nodes
                }
                Purpose::Objective => {
                    limits.max_objective_formula_nodes = cap;
                    FormulaResource::ObjectiveFormulaNodes
                }
            };
            with_builder(&limits, purpose, |builder| {
                let result = builder.initialize(location());
                if cap == 2 {
                    result.unwrap();
                    assert_eq!(builder.nodes.view().node(FALSUM).unwrap(), Node::False);
                    assert_eq!(
                        builder.nodes.view().node(VERUM).unwrap(),
                        Node::Implies(FALSUM, FALSUM)
                    );
                } else {
                    let Err(FormulaFailure::Limit {
                        resource: actual,
                        limit,
                        observed,
                        location: origin,
                    }) = result
                    else {
                        panic!("expected a located node ceiling: {result:?}");
                    };
                    assert_eq!(actual, resource);
                    assert_eq!(limit, cap as u128);
                    assert_eq!(observed, cap as u128 + 1);
                    assert_eq!(origin, location());
                    assert_eq!(builder.nodes.view().len(), cap);
                }
            });
        }
    }
}

#[test]
fn initialization_obeys_work_ceiling() {
    Fixture::default().with(location(), |_, computation, counters| {
        let before = counters.accounting.work;
        let limits = FormulaLimits {
            max_work: before + 1,
            ..FormulaLimits::default()
        };
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut builder = Builder::empty(
            computation,
            &limits,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            location(),
        )
        .unwrap();
        let result = builder.initialize(location());
        assert!(matches!(result, Err(FormulaFailure::Limit {
            resource: FormulaResource::Work, limit, observed, ..
        }) if limit == u128::from(before + 1) && observed == u128::from(before + 2)));
        assert_eq!(builder.counters.accounting.work - before, 1);
        assert!(
            builder.nodes.view().is_empty(),
            "the refused hash scan publishes no node"
        );
    });
}

#[test]
fn scoped_initialization_retains_spent_work() {
    for purpose in [Purpose::Objective, Purpose::Validation] {
        Fixture::default().with(location(), |support, computation, counters| {
            let mut limits = FormulaLimits {
                max_objective_formula_nodes: 1,
                ..FormulaLimits::default()
            };
            limits.theory.max_nodes = 1;
            let mut budget = Budget::new(ExpansionLimits::default(), 0);
            let binding = Binding::new(computation, &limits, counters, location()).unwrap();
            counters.charge_work(7, &limits, location()).unwrap();
            let before = counters.accounting.work;
            let mut context = Context {
                computation,
                limits: &limits,
                budget: &mut budget,
                counters,
                location: location(),
            };
            assert!(matches!(
                scoped_body::validate_with_purpose(&[], &binding, support, &mut context, purpose),
                Err(FormulaFailure::Limit {
                    limit: 1,
                    observed: 2,
                    ..
                })
            ));
            assert_eq!(
                counters.accounting.work - before,
                (1 + 2) + (1 + 4),
                "both node visits and their complete hashes remain spent"
            );
            counters.work(&limits, location()).unwrap();
            assert_eq!(counters.accounting.work - before, 9);
        });
    }
}

#[test]
fn producer_origins_preserve_atom_associations() {
    with_builder(&FormulaLimits::default(), Purpose::Theory, |builder| {
        builder.initialize(location()).unwrap();
        for name in ["p", "q"] {
            let atom =
                zetesis_core::Atom::new(zetesis_core::Predicate::new(name, 0).unwrap(), vec![])
                    .unwrap();
            builder.atom_ref((&atom).into(), location()).unwrap();
        }
        let origin = |offset| {
            ProgramSite::source(themelios_base::span::Location {
                source: SourceId::new(19),
                span: Span::empty(ByteOffset::new(offset)),
            })
        };
        let rule = |origins| RuleIr {
            head: HeadIr::Normal(None),
            body: vec![],
            body_variables: 0,
            bindings: None,
            variables: 0,
            location: location(),
            origins,
        };
        builder
            .record_head_origins(0, &rule(vec![origin(8), origin(2), origin(8)]))
            .unwrap();
        builder
            .record_head_origins(1, &rule(vec![origin(3)]))
            .unwrap();
        assert_eq!(
            builder.metadata.origins(0).collect::<Vec<_>>(),
            [location(), origin(2), origin(8)]
        );
        assert_eq!(
            builder.metadata.origins(1).collect::<Vec<_>>(),
            [location(), origin(3)]
        );
        let expected = (0..2)
            .map(|atom| builder.metadata.origins(atom).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        builder.support_guards().unwrap();
        assert_eq!(builder.origins, expected);
    });
}

#[test]
fn owned_root_provenance_transfers_its_storage() {
    with_builder(&FormulaLimits::default(), Purpose::Theory, |builder| {
        let origins = vec![location()];
        let data = origins.as_ptr();
        builder
            .root_at(FALSUM, Cow::Owned(origins), location())
            .unwrap();
        assert!(std::ptr::eq(builder.origins[0].as_ptr(), data));
    });
}

#[test]
fn origin_insertion_obeys_the_work_ceiling() {
    let next = ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(20),
        span: Span::empty(ByteOffset::new(0)),
    });
    let rule = RuleIr {
        head: HeadIr::Normal(None),
        body: vec![],
        body_variables: 0,
        bindings: None,
        variables: 0,
        location: next,
        origins: vec![next],
    };
    for allowance in [1, 2] {
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits { max_work: counters.accounting.work + allowance, ..FormulaLimits::default() };
            let mut budget = Budget::new(ExpansionLimits::default(), 0);
            let mut builder = Builder::empty(computation, &limits, &mut budget, counters,
                Purpose::Theory, None, location()).unwrap();
            builder.metadata.atom(location(), &mut builder.counters, builder.limits).unwrap();
            let before = builder.counters.accounting.work;
            let result = builder.record_head_origins(0, &rule);
            if allowance == 2 {
                result.unwrap();
                assert_eq!(builder.counters.accounting.work - before, 2);
            } else {
                assert!(matches!(result, Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work, observed, limit, location: found,
                }) if observed == u128::from(before + 2) && limit == u128::from(before + 1) && found == next));
                assert_eq!(builder.metadata.origins(0).collect::<Vec<_>>(), [location()]);
            }
        });
    }
}

#[test]
fn guard_evidence_admission_precedes_copy_work() {
    for resource in [FormulaResource::Roots, FormulaResource::Origins] {
        let mut limits = FormulaLimits::default();
        match resource {
            FormulaResource::Roots => limits.theory.max_roots = 0,
            FormulaResource::Origins => limits.max_origin_locations = 0,
            _ => unreachable!("the two emitted-evidence bounds"),
        }
        with_builder(&limits, Purpose::Theory, |builder| {
            builder.initialize(location()).unwrap();
            let atom =
                zetesis_core::Atom::new(zetesis_core::Predicate::new("p", 0).unwrap(), vec![])
                    .unwrap();
            builder.atom_ref((&atom).into(), location()).unwrap();
            let before = builder.counters.accounting.work;
            assert!(
                matches!(builder.support_guards(), Err(FormulaFailure::Limit { resource: found, .. }) if found == resource)
            );
            // The atom hit costs one visit, three hash words, a bucket visit,
            // and equality. Each implication costs a visit, four hash words,
            // and two child checks. Growing the three-entry bucket table and
            // four-entry link vector adds seven moves. No origin is copied.
            let atom_hit = 1 + 3 + 1 + 1;
            let implication = 1 + 4 + 2;
            assert_eq!(
                builder.counters.accounting.work - before,
                atom_hit + 3 * implication + 3 + 4
            );
            assert!(builder.roots.is_empty());
            assert!(builder.origins.is_empty());
            assert_eq!(builder.origin_count, 0);
        });
    }
}

#[test]
fn interleaved_producers_retain_the_support_fold() {
    with_builder(&FormulaLimits::default(), Purpose::Theory, |builder| {
        builder.initialize(location()).unwrap();
        let mut nodes = Vec::new();
        for name in ["p", "q", "x", "y", "z"] {
            let atom =
                zetesis_core::Atom::new(zetesis_core::Predicate::new(name, 0).unwrap(), vec![])
                    .unwrap();
            nodes.push(builder.atom_ref((&atom).into(), location()).unwrap());
        }
        let rule = RuleIr {
            head: HeadIr::Normal(None),
            body: vec![],
            body_variables: 0,
            bindings: None,
            variables: 0,
            location: location(),
            origins: vec![location()],
        };
        for (head, body) in [
            (nodes[0], nodes[2]),
            (nodes[1], nodes[4]),
            (nodes[0], nodes[3]),
            (nodes[0], nodes[2]),
        ] {
            builder.producer(head, body, &rule).unwrap();
        }
        let before = builder.nodes.view().len();
        builder.support_guards().unwrap();
        assert_eq!(
            builder.nodes.view().node(before).unwrap(),
            Node::Or(&[nodes[2], nodes[3]])
        );
        assert_eq!(
            builder.nodes.view().node(before + 1).unwrap(),
            Node::Or(&[before, nodes[2]])
        );
        assert_eq!(
            builder.nodes.view().node(before + 2).unwrap(),
            Node::Implies(nodes[0], before + 1)
        );
        assert_eq!(
            builder.roots.len(),
            5,
            "every atom retains its necessary guard"
        );
    });
}

#[test]
fn unchanged_groups_charge_reads_without_copying() {
    with_builder(&FormulaLimits::default(), Purpose::Theory, |builder| {
        builder.initialize(location()).unwrap();
        let atoms = [0, 1, 2].map(|atom| builder.node(Node::Atom(atom), location()).unwrap());
        for conjunction in [false, true] {
            // The repeated last atom is not adjacent; its occurrence is retained.
            let row = [atoms[0], atoms[1], atoms[2], atoms[0]];
            let node = if conjunction {
                Node::And(&row)
            } else {
                Node::Or(&row)
            };
            let expected = builder.node(node, location()).unwrap();
            let before = builder.counters.accounting.work;
            assert_eq!(builder.node(node, location()).unwrap(), expected);
            let lookup = builder.counters.accounting.work - before;
            let before = builder.counters.accounting.work;
            assert_eq!(
                builder.group(&row, conjunction, location()).unwrap(),
                expected
            );
            assert_eq!(
                builder.counters.accounting.work - before,
                lookup + row.len() as u64
            );
        }
    });
}

fn remapped_theories(conjunction: bool, codes: &[usize]) -> (Theory, Theory) {
    with_builder(&FormulaLimits::default(), Purpose::Theory, |builder| {
        builder.initialize(location()).unwrap();
        let p = builder.node(Node::Atom(0), location()).unwrap();
        let q = builder.node(Node::Atom(1), location()).unwrap();
        let not_p = builder.neg(p, location()).unwrap();
        let mapping = [q, not_p, VERUM, p, FALSUM];
        let row: Vec<_> = codes.iter().map(|&code| mapping[code]).collect();
        let original = match row.as_slice() {
            [] => boolean(conjunction),
            [single] => *single,
            _ => builder
                .node(
                    if conjunction {
                        Node::And(&row)
                    } else {
                        Node::Or(&row)
                    },
                    location(),
                )
                .unwrap(),
        };
        let normalized = builder
            .mapped_group(codes, conjunction, |code| mapping[code], location())
            .unwrap();
        let parts = std::mem::take(&mut builder.nodes).into_parts();
        let copy = FormulaParts::new(parts.nodes().to_vec(), parts.operands().to_vec()).unwrap();
        (
            Theory::new(2, parts, vec![original], builder.limits.theory).unwrap(),
            Theory::new(2, copy, vec![normalized], builder.limits.theory).unwrap(),
        )
    })
}

const GROUP_CASES: &[&[usize]] = &[
    &[],
    &[2],
    &[2, 2],
    &[2, 3, 2],
    &[2, 1, 2, 3],
    &[2, 0, 2, 3],
    &[2, 3, 4, 1, 4, 2],
    &[0, 2, 3],
    &[1, 2, 3],
    &[2, 3, 0],
    &[2, 3, 1],
];

#[test]
fn remapped_groups_preserve_original_satisfaction() {
    for conjunction in [false, true] {
        for &codes in GROUP_CASES {
            let (original, normalized) = remapped_theories(conjunction, codes);
            for atoms in [vec![], vec![0], vec![1], vec![0, 1]] {
                let evaluate = |theory: &Theory| {
                    models(
                        theory,
                        &Interpretation::new(theory, atoms.clone()).unwrap(),
                        Limits::default(),
                        &Cancellation::default(),
                    )
                    .unwrap()
                };
                assert_eq!(evaluate(&original), evaluate(&normalized));
            }
        }
    }
}

#[test]
fn remapped_groups_preserve_frozen_satisfaction() {
    for conjunction in [false, true] {
        for &codes in GROUP_CASES {
            let (original, normalized) = remapped_theories(conjunction, codes);
            for candidate in [vec![], vec![0], vec![1], vec![0, 1]] {
                for tested in [vec![], vec![0], vec![1], vec![0, 1]] {
                    let evaluate = |theory: &Theory| {
                        models_reduct(
                            theory,
                            &Interpretation::new(theory, candidate.clone()).unwrap(),
                            &Interpretation::new(theory, tested.clone()).unwrap(),
                            Limits::default(),
                            &Cancellation::default(),
                        )
                        .unwrap()
                    };
                    assert_eq!(evaluate(&original), evaluate(&normalized));
                }
            }
        }
    }
}

#[test]
fn stopped_group_scan_publishes_no_node() {
    for changed in [false, true] {
        with_builder(&FormulaLimits::default(), Purpose::Theory, |builder| {
            builder.initialize(location()).unwrap();
            let atoms = [0, 1, 2].map(|atom| builder.node(Node::Atom(atom), location()).unwrap());
            let row = [atoms[0], atoms[1], atoms[2], atoms[0]];
            let nodes = builder.nodes.view().len();
            let remaining = builder.limits.max_work - builder.counters.accounting.work - 2;
            builder
                .counters
                .charge_work(remaining.into(), builder.limits, location())
                .unwrap();
            let result = builder.mapped_group(
                &row,
                true,
                |value| {
                    if changed && value == atoms[1] {
                        VERUM
                    } else {
                        value
                    }
                },
                location(),
            );
            assert!(matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
            assert_eq!(builder.nodes.view().len(), nodes);
        });
    }
}

#[test]
fn native_suffix_remaps_every_immediate_operand() {
    with_builder(&FormulaLimits::default(), Purpose::Theory, |builder| {
        builder.initialize(location()).unwrap();
        let atoms = [0, 1, 2].map(|atom| builder.node(Node::Atom(atom), location()).unwrap());
        let row = [atoms[0], atoms[1], atoms[2], atoms[0]];
        let existing = builder.node(Node::Or(&row), location()).unwrap();
        let first = builder.nodes.view().len();
        let mut transaction = builder.nodes.transaction();
        let duplicate = transaction
            .push(Node::Or(&row), usize::MAX, usize::MAX)
            .unwrap();
        transaction
            .push(
                Node::And(&[duplicate, atoms[2], atoms[1]]),
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
        let suffix = transaction.detach().unwrap();
        assert_eq!(suffix.first(), first);
        let mapped = builder.intern_appended(&suffix, location()).unwrap();
        assert_eq!(mapped.slice()[0], existing);
        assert_eq!(
            builder.nodes.view().node(mapped.slice()[1]).unwrap(),
            Node::And(&[existing, atoms[2], atoms[1]])
        );
        assert_eq!(builder.nodes.view().len(), first + 1);
        assert_eq!(
            builder.nodes.parts().operands(),
            [
                atoms[0], atoms[1], atoms[2], atoms[0], existing, atoms[2], atoms[1]
            ]
        );
    });
}

#[test]
fn initialization_obeys_operand_ceiling() {
    for purpose in [Purpose::Theory, Purpose::Objective, Purpose::Validation] {
        for cap in 0..=2 {
            let mut limits = FormulaLimits::default();
            let resource = if matches!(purpose, Purpose::Objective) {
                limits.max_objective_formula_operands = cap;
                FormulaResource::ObjectiveFormulaOperands
            } else {
                limits.theory.max_operands = cap;
                FormulaResource::Operands
            };
            with_builder(&limits, purpose, |builder| {
                let result = builder.initialize(location());
                if cap == 2 {
                    result.unwrap();
                } else {
                    assert!(
                        matches!(result, Err(FormulaFailure::Limit { resource: found, limit, observed: 2, location: site }) if found == resource && limit == cap as u128 && site == location())
                    );
                    assert_eq!(builder.nodes.view().len(), 1);
                    assert_eq!(builder.nodes.parts().occurrences(), 0);
                    assert!(builder.nodes.parts().operands().is_empty());
                }
            });
        }
    }
}

/// The empty catalog leaves final admission as finish's only charged action.
/// Both routes retain the same native row and roots; raw re-entry discards only
/// the private topology frontier, without changing the source atom universe.
fn final_admission(
    retained: bool,
    max_work: u64,
    cancel_at_finish: bool,
) -> Result<(Theory, u64, u128), FormulaFailure> {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits {
            max_work,
            ..FormulaLimits::default()
        };
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut builder = Builder::empty(
            computation,
            &limits,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            location(),
        )?;
        builder.initialize(location())?;
        let root = builder.node(Node::Or(&[FALSUM, VERUM, FALSUM]), location())?;
        builder.root_at(root, Cow::Borrowed(&[]), location())?;
        if !retained {
            builder.nodes = FormulaNodes::new(std::mem::take(&mut builder.nodes).into_parts());
        }
        if cancel_at_finish {
            let cancellation = Cancellation::default();
            cancellation.cancel();
            builder.counters = builder.counters.with_cancellation(Some(&cancellation));
        }
        let before = builder.counters.accounting.work;
        let (emission, counters) = builder.finish(&Profile::new(None), location())?;
        let work = emission.admission.work();
        assert_eq!(u128::from(counters.accounting.work - before), work);
        let theory = emission.admission.admit().unwrap();
        Ok((theory, counters.accounting.work, work))
    })
}

#[test]
fn final_admission_charges_only_remaining_scans() {
    let (raw, raw_total, raw_work) = final_admission(false, u64::MAX, false).unwrap();
    let (checked, checked_total, checked_work) = final_admission(true, u64::MAX, false).unwrap();
    assert_eq!(raw.nodes(), checked.nodes());
    assert_eq!(raw.operands(), checked.operands());
    assert_eq!(raw.roots(), checked.roots());
    assert_eq!(
        checked_work,
        2 * checked.nodes().len() as u128 + checked.roots().len() as u128
    );
    let saved = raw.parts().occurrences() as u128;
    assert!(saved > 0);
    assert_eq!(raw_work - checked_work, saved);
    assert_eq!(u128::from(raw_total - checked_total), saved);
}

#[test]
fn final_admission_obeys_the_work_ceiling() {
    for retained in [false, true] {
        let (_, exact, _) = final_admission(retained, u64::MAX, false).unwrap();
        final_admission(retained, exact, false).unwrap();
        assert!(matches!(final_admission(retained, exact - 1, false),
            Err(FormulaFailure::Limit { resource: FormulaResource::Work, limit, observed, location: origin })
                if limit == u128::from(exact - 1) && observed == u128::from(exact) && origin == location()));
    }
}

#[test]
fn final_admission_keeps_cancellation_typed() {
    for retained in [false, true] {
        assert!(matches!(final_admission(retained, u64::MAX, true),
            Err(FormulaFailure::Interrupted { reason: zetesis_cpu::Stop::Cancelled, location: origin })
                if origin == location()));
    }
}
