//! Complete objective observations through total predicate renamings.

use crate::support::source_cases;

use oracle::records as clingo;
use source_cases::cases;
use zetesis_clingo_support as oracle;
use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Node, Theory};
use zetesis_reference_support::{admit, canonical, exhaustive};
use zetesis_test_support::records::Records;
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource,
    prepare_formula,
};

const FIXTURE: &str = include_str!("../fixtures/objective-forwarding.jsonl");
const EXTREMA_REFUSALS: &str = include_str!("../fixtures/objective-extrema-refusals.jsonl");
const RECURSIVE_COUNT: &str = "n(N):-N=#count{1:p(X)}.p(X):-n(X).#minimize{X:p(X)}.";

#[test]
fn forwarding_preserves_complete_model_cost_records() {
    let cases = cases(FIXTURE);
    assert_eq!(cases.len(), 38);
    let mut models = 0;
    for case in cases {
        let input = admit(&case.source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        let records = exhaustive(&input);
        assert_eq!(records, case.records, "{}: {}", case.name, case.source);
        models += records.len();
    }
    assert_eq!(models, 69);
}

const SIGNED_BASE: &str = "{p}.value(N):-N=#count{1:p}.copied(N):-value(N).";
const SIGNED_OBJECTIVE: &str = "#minimize{N:copied(N)}.";

#[test]
fn signed_choice_occurrences_do_not_define_observers() {
    let baseline = admit(
        &format!("{SIGNED_BASE}{SIGNED_OBJECTIVE}"),
        &FormulaLimits::default(),
    )
    .unwrap();
    let expected = exhaustive(&baseline);
    for choice in [
        "{not copied(0)}.",
        "{not not copied(0)}.",
        "{not copied(0);not not copied(1)}.",
    ] {
        let source = format!("{SIGNED_BASE}{choice}{SIGNED_OBJECTIVE}");
        let input = admit(&source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(exhaustive(&input), expected, "{source}");
    }
}

#[test]
fn signed_choices_preserve_assignment_dependencies() {
    let base = "{p}.value(N):-N=#count{1:p}.copied(N):-N=#sum{X:value(X)}.";
    let baseline = admit(
        &format!("{base}{SIGNED_OBJECTIVE}"),
        &FormulaLimits::default(),
    )
    .unwrap();
    let expected = exhaustive(&baseline);
    for choice in ["{not copied(0)}.", "{not not copied(0)}."] {
        let source = format!("{base}{choice}{SIGNED_OBJECTIVE}");
        let input = admit(&source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(exhaustive(&input), expected, "{source}");
    }
}

#[test]
fn signed_choice_bounds_still_filter_observed_answers() {
    for (choice, required) in [
        ("1{not copied(0)}1.", "p"),
        ("1{not not copied(0)}1.", "copied(0)"),
    ] {
        let source = format!("{SIGNED_BASE}{choice}{SIGNED_OBJECTIVE}");
        let input = admit(&source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        let records = exhaustive(&input);
        assert_eq!(records.len(), 1, "{source}");
        assert!(
            records.iter().all(|(atoms, _)| atoms.contains(required)),
            "{source}"
        );
    }
}

#[test]
#[ignore = "requires independently installed clingo"]
fn signed_observer_sources_match_clingo_costs() {
    for base in [
        SIGNED_BASE,
        "{p}.value(N):-N=#count{1:p}.copied(N):-N=#sum{X:value(X)}.",
    ] {
        for choice in [
            "",
            "{not copied(0)}.",
            "{not not copied(0)}.",
            "1{not copied(0)}1.",
            "1{not not copied(0)}1.",
        ] {
            let source = format!("{base}{choice}{SIGNED_OBJECTIVE}");
            let input = admit(&source, &FormulaLimits::default()).unwrap();
            assert_eq!(exhaustive(&input), clingo(&source), "{source}");
        }
    }
}

#[test]
fn search_preserves_every_optimum_tie() {
    for case in cases(FIXTURE) {
        let input = admit(&case.source, &FormulaLimits::default()).unwrap();
        let mut search = zetesis_sat::StableModels::new(
            input.theory(),
            zetesis_sat::Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        let mut records = Records::new();
        for interpretation in search.by_ref() {
            let interpretation = interpretation.unwrap();
            let atoms: Vec<_> = interpretation
                .atoms()
                .map(|index| input.atoms().at(index).unwrap())
                .collect();
            let evaluated_model =
                Model::from_positions(input.atom_catalog(), interpretation.atoms()).unwrap();
            let evaluation = zetesis_objective::evaluate(
                input.objectives(),
                &evaluated_model,
                zetesis_objective::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            let score = evaluation.score();
            let costs = score
                .is_present()
                .then(|| score.costs().iter().map(|&(_, value)| value).collect());
            assert!(records.insert((atoms.iter().copied().map(canonical).collect(), costs)));
        }
        assert!(search.exhausted(), "{}: complete reduct search", case.name);
        // These fixed-alphabet fixtures have the same priority layout in every
        // model. Compare all minimizers, including distinct models with zero cost.
        assert_eq!(optima(&records), optima(&case.records), "{}", case.name);
    }
}

fn optima(records: &Records) -> Records {
    let Some(best) = records.iter().map(|(_, costs)| costs).min() else {
        return Records::new();
    };
    records
        .iter()
        .filter(|(_, costs)| costs == best)
        .cloned()
        .collect()
}

fn truth(theory: &Theory, mask: usize, frozen: Option<&[bool]>) -> Vec<bool> {
    let mut values = Vec::new();
    for (index, node) in theory.nodes().iter().enumerate() {
        let value = match *node {
            Node::False => false,
            Node::Atom(atom) => mask & (1 << atom) != 0,
            Node::And(left, right) => values[left] && values[right],
            Node::Or(left, right) => values[left] || values[right],
            Node::Implies(left, right) => !values[left] || values[right],
        };
        values.push(value && frozen.is_none_or(|outer| outer[index]));
    }
    values
}

fn holds(theory: &Theory, values: &[bool]) -> bool {
    theory.roots().iter().all(|&root| values[root])
}

#[test]
fn frozen_forwarding_keeps_assignment_equalities() {
    let source = "{a}.n(N):-N=#count{1:a}.p(X):-n(X).#minimize{X@7:p(X)}.";
    let expanded = "{a}.n(0):-not a.n(1):-a.p(0):-n(0).p(1):-n(1).";
    let input = admit(source, &FormulaLimits::default()).unwrap();
    let reference = admit(expanded, &FormulaLimits::default()).unwrap();
    let count = input.atoms().len();
    assert_eq!(count, 5, "complete proposed carrier");
    assert_eq!(input.atoms(), reference.atoms());
    let mut pairs = 0;
    for outer in 0..1_usize << count {
        let left = truth(input.theory(), outer, None);
        let right = truth(reference.theory(), outer, None);
        let mut inner = outer;
        loop {
            assert_eq!(
                holds(input.theory(), &truth(input.theory(), inner, Some(&left))),
                holds(
                    reference.theory(),
                    &truth(reference.theory(), inner, Some(&right))
                ),
                "M={outer}, J={inner}"
            );
            pairs += 1;
            if inner == 0 {
                break;
            }
            inner = (inner - 1) & outer;
        }
    }
    assert_eq!(pairs, 243);
}

#[test]
fn forwarded_observers_leave_the_original_theory_intact() {
    for source in [
        "{a}.n(N):-N=#count{1:a}.p(X):-n(X).q(Y):-p(Y).#minimize{Y@7:q(Y)}.",
        RECURSIVE_COUNT,
    ] {
        let (program, _) = source.split_once("#minimize").unwrap();
        let ordinary = admit(program, &FormulaLimits::default()).unwrap();
        let observed = admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(ordinary.atoms(), observed.atoms());
        assert_eq!(ordinary.theory().nodes(), observed.theory().nodes());
        assert_eq!(ordinary.theory().roots(), observed.theory().roots());
        assert_eq!(ordinary.formula_origins(), observed.formula_origins());
    }
}

#[test]
fn recursive_count_forwarding_has_no_answer_set() {
    let input = admit(RECURSIVE_COUNT, &FormulaLimits::default()).unwrap();
    assert_eq!(input.objectives().priorities(), [0]);
    // With no p, count zero forces n(0) and p(0). With any p, the
    // unique tuple key has count one, leaving only the unsupported
    // positive n(1)/p(1) cycle in the reduct. Neither is an answer set.
    assert_eq!(exhaustive(&input), Records::new());
}

#[test]
#[ignore = "requires independently installed clingo"]
fn recursive_count_forwarding_matches_clingo() {
    assert_eq!(clingo(RECURSIVE_COUNT), Records::new());
}

#[test]
fn preparation_defers_carrier_presence_to_grounding() {
    let source =
        "b.{a}.n(N):-N=#max{2:a;foo:b}.p(X):-n(X).m(M):-M=#max{Y:p(Y)}.#minimize{M@7:m(M)}.";
    let prepared = prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let input = prepared.ground().unwrap();
    assert_eq!(input.objectives().priorities(), &[7]);
    let records = exhaustive(&input);
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|(_, costs)| *costs == Some(vec![0])));
}

#[derive(Clone, Copy)]
enum Resource {
    Work,
    Substitutions,
    SupportRounds,
    AnalysisNodes,
    AnalysisEdges,
}

impl Resource {
    fn limits(self, value: u64) -> FormulaLimits {
        let mut limits = FormulaLimits::default();
        match self {
            Self::Work => limits.max_work = value,
            Self::Substitutions => limits.max_substitutions = value,
            Self::SupportRounds => limits.max_support_rounds = value,
            Self::AnalysisNodes => limits.max_analysis_nodes = usize::try_from(value).unwrap(),
            Self::AnalysisEdges => limits.max_analysis_edges = usize::try_from(value).unwrap(),
        }
        limits
    }

    fn kind(self) -> FormulaResource {
        match self {
            Self::Work => FormulaResource::Work,
            Self::Substitutions => FormulaResource::Substitutions,
            Self::SupportRounds => FormulaResource::SupportRounds,
            Self::AnalysisNodes => FormulaResource::AnalysisNodes,
            Self::AnalysisEdges => FormulaResource::AnalysisEdges,
        }
    }
}

#[test]
fn exact_admission_limits_preserve_objective_completion() {
    for source in [
        "{a}.n(N):-N=#count{1:a}.p(X):-n(X).q(Y):-p(Y).#minimize{Y@7:q(Y)}.",
        "{a}.n(N):-N=#max{2:a}.p(X):-n(X).#minimize{X@7:p(X)}.",
    ] {
        exact_limits(source);
    }
}

fn exact_limits(source: &str) {
    let reference = admit(source, &FormulaLimits::default()).unwrap();
    for resource in [
        Resource::Work,
        Resource::Substitutions,
        Resource::SupportRounds,
        Resource::AnalysisNodes,
        Resource::AnalysisEdges,
    ] {
        let mut lower = 0;
        let mut upper = 10_000;
        assert!(admit(source, &resource.limits(upper)).is_ok());
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if admit(source, &resource.limits(middle)).is_ok() {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(lower > 0);
        let exact = admit(source, &resource.limits(lower)).unwrap();
        assert_eq!(exhaustive(&exact), exhaustive(&reference));
        let error = admit(source, &resource.limits(lower - 1)).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Limit { resource: actual, observed, limit, .. }
                if actual == resource.kind() && observed > limit),
            "{error}"
        );
        assert!(!error.diagnostics().is_empty());
        assert_eq!(
            exhaustive(&admit(source, &resource.limits(lower)).unwrap()),
            exhaustive(&reference),
            "retry publishes the complete objective contract"
        );
    }
}

#[test]
#[ignore = "requires independent clingo; 48 original model/cost references"]
fn forwarded_observers_match_fresh_clingo() {
    for case in cases(FIXTURE).into_iter().chain(cases(EXTREMA_REFUSALS)) {
        assert_eq!(clingo(&case.source), case.records, "{}", case.name);
    }
}
