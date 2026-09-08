//! Finite conditional-premise laws and bounded candidate-only composition.

use proptest::prelude::*;
use zetesis_cpu::Stop;
use zetesis_ferraris::{Interpretation, models};
use zetesis_sat::{
    Control,
    partition::{
        ErrorKind, Group, Limits, Plan, Premises, Resource, RestrictionErrorKind, RestrictionLimits,
    },
};

fn plan(members: &[usize], groups: &[Group<'_>], lower: usize) -> Plan {
    Plan::new(
        Premises {
            atom_count: 8,
            members,
            lower,
            groups,
        },
        Limits::default(),
        &Control::default(),
    )
    .unwrap()
}

#[test]
fn saturated_disjoint_capacities_force_local_counts() {
    let groups = [
        Group {
            members: &[5, 1],
            upper: 1,
        },
        Group {
            members: &[7, 2, 3],
            upper: 2,
        },
    ];
    let plan = plan(&[1, 2, 3, 5, 7], &groups, 3);
    let bounds: Vec<_> = plan
        .consequences()
        .map(|group| (group.members, group.lower, group.upper))
        .collect();
    assert_eq!(bounds, [(&[5, 1][..], 1, 1), (&[7, 2, 3][..], 2, 2)]);
}

#[test]
fn loose_capacities_cannot_exceed_distinct_members() {
    let groups = [
        Group {
            members: &[0, 1],
            upper: usize::MAX,
        },
        Group {
            members: &[],
            upper: usize::MAX,
        },
    ];
    let plan = plan(&[0, 1], &groups, 1);
    assert_eq!(plan.capacity(), 2);
    assert_eq!(
        plan.consequences()
            .map(|group| group.lower)
            .collect::<Vec<_>>(),
        [1, 0]
    );
}

#[test]
fn contradictory_premises_emit_falsum() {
    for (members, groups) in [
        (&[][..], vec![]),
        (
            &[0][..],
            vec![Group {
                members: &[0],
                upper: 0,
            }],
        ),
    ] {
        let plan = plan(members, &groups, 1);
        assert!(plan.inconsistent());
        let restriction = plan
            .restriction(RestrictionLimits::default(), &Control::default())
            .unwrap();
        for selected in [vec![], vec![0]] {
            let candidate = Interpretation::new(restriction.theory(), selected).unwrap();
            assert!(
                !models(
                    restriction.theory(),
                    &candidate,
                    zetesis_ferraris::Limits::default(),
                    &Control::default()
                )
                .unwrap()
            );
        }
    }
}

#[test]
fn incomplete_partitions_are_refused() {
    let groups = [Group {
        members: &[0],
        upper: 1,
    }];
    let error = Plan::new(
        Premises {
            atom_count: 2,
            members: &[0, 1],
            lower: 1,
            groups: &groups,
        },
        Limits::default(),
        &Control::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MissingMember(1));
}

#[test]
fn overlapping_partitions_are_refused() {
    for groups in [
        vec![Group {
            members: &[0, 0],
            upper: 1,
        }],
        vec![
            Group {
                members: &[0],
                upper: 1
            };
            2
        ],
    ] {
        let error = Plan::new(
            Premises {
                atom_count: 1,
                members: &[0],
                lower: 1,
                groups: &groups,
            },
            Limits::default(),
            &Control::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::RepeatedMember(0));
    }
}

#[test]
fn undeclared_group_members_are_refused() {
    let groups = [Group {
        members: &[0, 1],
        upper: 1,
    }];
    let error = Plan::new(
        Premises {
            atom_count: 2,
            members: &[0],
            lower: 1,
            groups: &groups,
        },
        Limits::default(),
        &Control::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ForeignMember(1));
}

#[test]
fn duplicate_declarations_are_refused() {
    let error = Plan::new(
        Premises {
            atom_count: 2,
            members: &[0, 0],
            lower: 0,
            groups: &[],
        },
        Limits::default(),
        &Control::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::DuplicateMember(0));
}

#[test]
fn out_of_universe_members_are_refused() {
    for (members, groups) in [
        (&[2][..], vec![]),
        (
            &[0][..],
            vec![Group {
                members: &[2],
                upper: 1,
            }],
        ),
    ] {
        let error = Plan::new(
            Premises {
                atom_count: 2,
                members,
                lower: 1,
                groups: &groups,
            },
            Limits::default(),
            &Control::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Atom(2));
    }
}

#[test]
fn shape_admission_requires_the_complete_work_budget() {
    let groups = [
        Group {
            members: &[0, 1],
            upper: 1,
        },
        Group {
            members: &[2],
            upper: 1,
        },
    ];
    let input = Premises {
        atom_count: 3,
        members: &[0, 1, 2],
        lower: 2,
        groups: &groups,
    };
    let full = Plan::new(input, Limits::default(), &Control::default()).unwrap();
    // Two group passes, coverage initialization, declaration, grouping and coverage validation.
    assert_eq!(full.statistics().work, 16);
    for maximum in 0..=full.statistics().work {
        let limits = Limits {
            max_work: maximum,
            ..Limits::default()
        };
        match Plan::new(input, limits, &Control::default()) {
            Ok(_) => assert_eq!(maximum, full.statistics().work),
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::Limit(Resource::Work));
                assert_eq!(error.statistics().work, maximum);
            }
        }
    }
}

#[test]
fn storage_admission_includes_transient_coverage() {
    let groups = [Group {
        members: &[0, 1],
        upper: 1,
    }];
    let input = Premises {
        atom_count: 5,
        members: &[0, 1],
        lower: 1,
        groups: &groups,
    };
    let full = Plan::new(input, Limits::default(), &Control::default()).unwrap();
    let bytes = full.statistics().construction_bytes;
    assert!(bytes > full.statistics().resident_bytes);
    let exact = Limits {
        max_bytes: bytes,
        ..Limits::default()
    };
    assert!(Plan::new(input, exact, &Control::default()).is_ok());
    let short = Limits {
        max_bytes: bytes - 1,
        ..exact
    };
    let error = Plan::new(input, short, &Control::default()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Limit(Resource::Bytes));
    assert_eq!(error.statistics().work, 1);
}

#[test]
fn membership_occurrences_have_an_independent_ceiling() {
    let groups = [Group {
        members: &[0, 0],
        upper: 1,
    }];
    let input = Premises {
        atom_count: 1,
        members: &[0],
        lower: 0,
        groups: &groups,
    };
    let error = Plan::new(
        input,
        Limits {
            max_members: 1,
            ..Limits::default()
        },
        &Control::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Limit(Resource::Members));
}

#[test]
fn shape_dimensions_are_checked_before_allocation() {
    let groups = [Group {
        members: &[0, 1],
        upper: 1,
    }];
    let input = Premises {
        atom_count: 3,
        members: &[0, 1],
        lower: 1,
        groups: &groups,
    };
    for (limits, resource) in [
        (
            Limits {
                max_atoms: 2,
                ..Limits::default()
            },
            Resource::Atoms,
        ),
        (
            Limits {
                max_members: 1,
                ..Limits::default()
            },
            Resource::Members,
        ),
        (
            Limits {
                max_groups: 0,
                ..Limits::default()
            },
            Resource::Groups,
        ),
    ] {
        let error = Plan::new(input, limits, &Control::default()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Limit(resource));
        assert_eq!(
            error.statistics(),
            zetesis_sat::partition::Statistics::default()
        );
    }
}

#[test]
fn emission_stops_before_allocating_an_oversized_group() {
    let groups = [Group {
        members: &[0, 1],
        upper: 1,
    }];
    let plan = plan(&[0, 1], &groups, 1);
    let mut limits = RestrictionLimits::default();
    limits.aggregate.max_elements = 1;
    let error = plan.restriction(limits, &Control::default()).unwrap_err();
    assert_eq!(error.kind(), RestrictionErrorKind::Elements);
    assert_eq!(error.work(), 1);
}

#[test]
fn emission_refuses_partial_root_coverage() {
    let groups = [
        Group {
            members: &[0, 1],
            upper: 1,
        },
        Group {
            members: &[2, 3],
            upper: 1,
        },
    ];
    let plan = plan(&[0, 1, 2, 3], &groups, 2);
    let mut limits = RestrictionLimits::default();
    limits.theory.max_roots = 1;
    let error = plan.restriction(limits, &Control::default()).unwrap_err();
    assert_eq!(error.kind(), RestrictionErrorKind::Roots);
    assert!(error.work() > 1);
    assert_eq!(
        plan.restriction(RestrictionLimits::default(), &Control::default())
            .unwrap()
            .theory()
            .roots()
            .len(),
        2
    );
}

#[test]
fn emission_obeys_its_final_node_ceiling() {
    let groups = [Group {
        members: &[0, 1],
        upper: 1,
    }];
    let plan = plan(&[0, 1], &groups, 1);
    let full = plan
        .restriction(RestrictionLimits::default(), &Control::default())
        .unwrap();
    let mut limits = RestrictionLimits::default();
    limits.theory.max_nodes = full.theory().nodes().len();
    assert!(plan.restriction(limits, &Control::default()).is_ok());
    limits.theory.max_nodes -= 1;
    let error = plan.restriction(limits, &Control::default()).unwrap_err();
    let RestrictionErrorKind::Aggregate(error) = error.kind() else {
        panic!("expected cardinality construction to reach its node ceiling");
    };
    assert_eq!(
        error.kind(),
        zetesis_ferraris::AggregateErrorKind::NodeLimit
    );
}

#[test]
fn emission_preserves_its_declared_atom_universe() {
    let plan = plan(&[], &[], 0);
    let mut limits = RestrictionLimits::default();
    limits.theory.max_atoms = 7;
    let error = plan.restriction(limits, &Control::default()).unwrap_err();
    assert_eq!(error.kind(), RestrictionErrorKind::Atoms);
    assert_eq!(error.work(), 0);
}

#[test]
fn cancelled_empty_admission_does_not_succeed() {
    let control = Control::default();
    control.cancel();
    let error = Plan::new(
        Premises {
            atom_count: 0,
            members: &[],
            lower: 0,
            groups: &[],
        },
        Limits::default(),
        &control,
    )
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Stopped(Stop::Cancelled));
    assert_eq!(error.statistics().work, 0);
}

#[test]
fn cancelled_emission_exposes_no_restriction() {
    let plan = plan(&[], &[], 0);
    let control = Control::default();
    control.cancel();
    let error = plan
        .restriction(RestrictionLimits::default(), &control)
        .unwrap_err();
    assert_eq!(error.kind(), RestrictionErrorKind::Stopped(Stop::Cancelled));
    assert_eq!(error.work(), 0);
}

#[test]
fn emission_requires_the_complete_cumulative_work_budget() {
    let groups = [
        Group {
            members: &[0, 1],
            upper: 1,
        },
        Group {
            members: &[2, 3],
            upper: 1,
        },
    ];
    let plan = plan(&[0, 1, 2, 3], &groups, 2);
    let full = plan
        .restriction(RestrictionLimits::default(), &Control::default())
        .unwrap();
    for maximum in 0..=full.work() {
        let limits = RestrictionLimits {
            max_work: maximum,
            ..RestrictionLimits::default()
        };
        match plan.restriction(limits, &Control::default()) {
            Ok(_) => assert_eq!(maximum, full.work()),
            Err(error) => assert_eq!(error.work(), maximum),
        }
    }
}

#[test]
fn lower_bound_emission_does_not_assert_upper_premises() {
    let groups = [Group {
        members: &[0, 1],
        upper: 1,
    }];
    let plan = plan(&[0, 1], &groups, 1);
    let restriction = plan
        .restriction(RestrictionLimits::default(), &Control::default())
        .unwrap();
    let candidate = Interpretation::new(restriction.theory(), [0, 1]).unwrap();
    assert!(
        models(
            restriction.theory(),
            &candidate,
            zetesis_ferraris::Limits::default(),
            &Control::default()
        )
        .unwrap()
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn every_model_of_stated_premises_satisfies_consequences(
        owners in prop::collection::vec(0_usize..4, 0..7),
        caps in prop::array::uniform4(0_usize..8),
        lower in 0_usize..9,
    ) {
        let members: Vec<_> = (0..owners.len()).collect();
        let rows: Vec<Vec<_>> = (0..4).map(|owner| members.iter().copied().filter(|&atom| owners[atom] == owner).collect()).collect();
        let groups: Vec<_> = rows.iter().zip(caps).map(|(row, upper)| Group { members: row, upper }).collect();
        let plan = plan(&members, &groups, lower);
        for mask in 0_usize..(1 << members.len()) {
            let count = |row: &[usize]| row.iter().filter(|&&atom| mask & (1 << atom) != 0).count();
            let premise = count(&members) >= lower && groups.iter().all(|group| count(group.members) <= group.upper);
            if premise {
                prop_assert!(!plan.inconsistent());
                prop_assert!(plan.consequences().all(|group| count(group.members) >= group.lower));
            }
        }
    }

    #[test]
    fn emitted_truth_matches_independent_derived_counts(
        sizes in prop::array::uniform3(0_usize..3),
        caps in prop::array::uniform3(0_usize..4),
        lower in 0_usize..8,
    ) {
        let mut next = 0;
        let rows: Vec<Vec<_>> = sizes.into_iter().map(|size| { let row = (next..next + size).collect(); next += size; row }).collect();
        let groups: Vec<_> = rows.iter().zip(caps).map(|(row, upper)| Group { members: row, upper }).collect();
        let members: Vec<_> = (0..next).collect();
        let plan = plan(&members, &groups, lower);
        let restriction = plan.restriction(RestrictionLimits::default(), &Control::default()).unwrap();
        for mask in 0_usize..(1 << next) {
            let selected: Vec<_> = members.iter().copied().filter(|atom| mask & (1 << atom) != 0).collect();
            let capacity: usize = groups.iter().map(|group| group.upper.min(group.members.len())).sum();
            let expected = lower <= capacity && groups.iter().enumerate().all(|(index, group)| {
                let outside: usize = groups.iter().enumerate().filter(|(other, _)| *other != index).map(|(_, other)| other.upper.min(other.members.len())).sum();
                selected.iter().filter(|atom| group.members.contains(atom)).count() >= lower.saturating_sub(outside)
            });
            let candidate = Interpretation::new(restriction.theory(), selected).unwrap();
            prop_assert_eq!(models(restriction.theory(), &candidate, zetesis_ferraris::Limits::default(), &Control::default()).unwrap(), expected);
        }
    }
}
