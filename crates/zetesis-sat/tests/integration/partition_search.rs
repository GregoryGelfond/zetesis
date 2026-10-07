//! Candidate-only consequences, with caller premises established independently.

use std::collections::BTreeSet;

use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
use zetesis_sat::{
    Cancellation, Limits, StableModels,
    partition::{Group, Plan, Premises, RestrictionLimits},
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn search(
    theory: &Theory,
    restriction: Option<&Theory>,
) -> (BTreeSet<Vec<usize>>, zetesis_sat::Statistics) {
    let mut search = StableModels::new(theory, Limits::default(), Cancellation::default()).unwrap();
    if let Some(restriction) = restriction {
        search.restrict_candidates(restriction).unwrap();
    }
    assert!(search.theory().same_instance(theory));
    let mut found = BTreeSet::new();
    for model in search.by_ref() {
        let model = model.unwrap();
        assert!(model.theory().same_instance(theory));
        assert!(found.insert(model.atoms().collect()));
    }
    assert!(search.exhausted());
    (found, search.statistics())
}

#[test]
fn candidate_consequences_preserve_independent_stability() {
    let admitted = admit_formula(
        "2 { a; b; c; d } 2. :- a,b. :- c,d.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let theory = admitted.theory();
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
    let premises = Premises {
        atom_count: 4,
        members: &[0, 1, 2, 3],
        lower: 2,
        groups: &groups,
    };
    // Check this fixture's atom-index correspondence rather than assuming the front end sorts it.
    assert_eq!(
        admitted
            .atoms()
            .iter()
            .map(|atom| atom.predicate().name())
            .collect::<Vec<_>>(),
        ["a", "b", "c", "d"]
    );
    let plan = Plan::new(
        premises,
        zetesis_sat::partition::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let restriction = plan
        .restriction(RestrictionLimits::default(), &Cancellation::default())
        .unwrap();
    let mut expected = BTreeSet::new();
    for mask in 0_usize..16 {
        let atoms: Vec<_> = (0..4).filter(|atom| mask & (1 << atom) != 0).collect();
        let candidate = Interpretation::new(theory, atoms.clone()).unwrap();
        let original = zetesis_ferraris::models(
            theory,
            &candidate,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        if original {
            assert_eq!(atoms.len(), 2);
            assert!(groups.iter().all(|group| {
                atoms
                    .iter()
                    .filter(|atom| group.members.contains(atom))
                    .count()
                    <= group.upper
            }));
        }
        // This exhaustive subset kernel is independent of native candidate search.
        if zetesis_ferraris::check(
            theory,
            &candidate,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .accepted()
        {
            expected.insert(atoms);
        }
    }
    assert_eq!(expected.len(), 4);
    assert_eq!(search(theory, Some(restriction.theory())).0, expected);
    assert_eq!(search(theory, None).0, expected);
}

#[test]
fn a_candidate_bound_does_not_supply_reduct_support() {
    let original = Theory::new(
        2,
        zetesis_ferraris::FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::implies(0, 1),
                Node::implies(1, 0),
            ],
            vec![],
        )
        .unwrap(),
        vec![2, 3],
        AdmissionLimits::default(),
    )
    .unwrap();
    let groups = [Group {
        members: &[0, 1],
        upper: 2,
    }];
    // These premises are deliberately NOT entailed by the original theory.
    // The resulting restricted region excludes its stable empty model, but
    // must not turn the unsupported positive cycle into a new stable model.
    let plan = Plan::new(
        Premises {
            atom_count: 2,
            members: &[0, 1],
            lower: 1,
            groups: &groups,
        },
        zetesis_sat::partition::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let restriction = plan
        .restriction(RestrictionLimits::default(), &Cancellation::default())
        .unwrap();
    let (models, statistics) = search(&original, Some(restriction.theory()));
    assert!(models.is_empty());
    assert_eq!(statistics.countermodels, 1);
}

#[test]
fn shape_validation_does_not_certify_theory_entailment() {
    let original = Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(vec![], vec![]).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let groups = [Group {
        members: &[0],
        upper: 1,
    }];
    let plan = Plan::new(
        Premises {
            atom_count: 1,
            members: &[0],
            lower: 1,
            groups: &groups,
        },
        zetesis_sat::partition::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let restriction = plan
        .restriction(RestrictionLimits::default(), &Cancellation::default())
        .unwrap();
    let empty = Interpretation::new(&original, []).unwrap();
    assert!(
        zetesis_ferraris::models(
            &original,
            &empty,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
    );
    let empty = Interpretation::new(restriction.theory(), []).unwrap();
    assert!(
        !zetesis_ferraris::models(
            restriction.theory(),
            &empty,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
    );
}

#[test]
fn inactive_capacity_cannot_justify_an_unconditional_bound() {
    let admitted = admit_formula(
        "2 { a; b; c; d } 2. { g }. :- g,a,b. :- g,c,d.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let members = ["a", "b", "c", "d"].map(|name| {
        admitted
            .atoms()
            .iter()
            .position(|atom| atom.predicate().name() == name)
            .unwrap()
    });
    let groups = [
        Group {
            members: &members[..2],
            upper: 1,
        },
        Group {
            members: &members[2..],
            upper: 1,
        },
    ];
    let plan = Plan::new(
        Premises {
            atom_count: 5,
            members: &members,
            lower: 2,
            groups: &groups,
        },
        zetesis_sat::partition::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let restriction = plan
        .restriction(RestrictionLimits::default(), &Cancellation::default())
        .unwrap();
    let original = Interpretation::new(admitted.theory(), members[..2].iter().copied()).unwrap();
    assert!(
        zetesis_ferraris::models(
            admitted.theory(),
            &original,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
    );
    let constrained =
        Interpretation::new(restriction.theory(), members[..2].iter().copied()).unwrap();
    assert!(
        !zetesis_ferraris::models(
            restriction.theory(),
            &constrained,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
    );
}
