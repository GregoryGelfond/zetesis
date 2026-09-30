//! Source-to-certificate boundaries; classification reads the completed DAG.

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    Interpretation, TightCheckLimits, TightError, TightPlan, TightPlanLimits, TightVerdict,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

#[test]
fn source_normal_choice_and_frozen_aggregate_guards_are_certified() {
    for (source, models) in [
        ("a.", 1),
        ("{a}.", 2),
        ("{a}. b :- a.", 2),
        ("1 { a; b } 1.", 2),
        ("{a;b}. :- #count{1:a;2:b} > 1.", 3),
        ("{a;b}. c :- #count{1:a;2:b} >= 1.", 4),
        ("{a;b}. c :- not #count{1:a;2:b} = 1.", 4),
        ("a :- not not a.", 2),
    ] {
        let admitted = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let theory = admitted.theory();
        let plan = TightPlan::compile(theory, TightPlanLimits::default(), &Cancellation::default())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        let mut accepted = 0;
        for mask in 0..1usize << theory.atom_count() {
            let candidate = Interpretation::new(
                theory,
                (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
            )
            .unwrap();
            let checked = plan
                .check(
                    &candidate,
                    TightCheckLimits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            let reference = zetesis_ferraris::check(
                theory,
                &candidate,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            assert_eq!(
                checked.verdict == TightVerdict::Stable,
                reference.accepted(),
                "{source}"
            );
            accepted += usize::from(checked.verdict == TightVerdict::Stable);
        }
        assert_eq!(accepted, models, "{source}");
    }
}

#[test]
fn source_class_labels_cannot_hide_positive_aggregate_or_conditional_dependencies() {
    for source in [
        "{p}. p :- #count{1:p} > 0.",
        "p :- p:p.",
        "a|b. a:-b. b:-a.",
    ] {
        let admitted = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let result = TightPlan::compile(
            admitted.theory(),
            TightPlanLimits::default(),
            &Cancellation::default(),
        );
        assert!(
            matches!(
                result,
                Err(TightError::PositiveCycle { .. }
                    | TightError::UnsupportedBody { .. }
                    | TightError::UnsupportedRoot { .. })
            ),
            "{source}: {result:?}"
        );
    }
}
