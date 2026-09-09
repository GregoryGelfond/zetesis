//! Complete objective observations through total predicate renamings.

#[path = "support/source_records.rs"]
mod reference;

use reference::{Records, admit, canonical, cases, clingo, exhaustive};
use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_ferraris::{Node, Theory};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, FormulaResource, ProfileFeature, prepare_formula,
};

const FIXTURE: &str = include_str!("fixtures/objective-forwarding.jsonl");
const EXTREMA_REFUSALS: &str = include_str!("fixtures/objective-extrema-refusals.jsonl");

#[test]
fn forwarding_preserves_complete_model_cost_records() {
    let cases = cases(FIXTURE);
    assert_eq!(cases.len(), 38);
    let mut models = 0;
    for case in cases {
        let input = admit(&case.source, FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        let records = exhaustive(&input);
        assert_eq!(records, case.records, "{}: {}", case.name, case.source);
        models += records.len();
    }
    assert_eq!(models, 69);
}

#[test]
fn search_preserves_every_optimum_tie() {
    for case in cases(FIXTURE) {
        let input = admit(&case.source, FormulaLimits::default()).unwrap();
        let mut search = zetesis_sat::StableModels::new(
            input.theory(),
            zetesis_sat::Limits::default(),
            Control::default(),
        )
        .unwrap();
        let mut records = Records::new();
        for interpretation in search.by_ref() {
            let interpretation = interpretation.unwrap();
            let atoms: Vec<_> = interpretation
                .atoms()
                .map(|index| input.atoms()[index].clone())
                .collect();
            let evaluation = zetesis_objective::evaluate(
                input.objectives(),
                &Model::new(atoms.iter().cloned()),
                zetesis_objective::Limits::default(),
                &Control::default(),
            )
            .unwrap();
            let score = evaluation.score();
            let costs = score
                .is_present()
                .then(|| score.costs().iter().map(|&(_, value)| value).collect());
            assert!(records.insert((atoms.iter().map(canonical).collect(), costs)));
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
    let input = admit(source, FormulaLimits::default()).unwrap();
    let reference = admit(expanded, FormulaLimits::default()).unwrap();
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
    let program = "{a}.n(N):-N=#count{1:a}.p(X):-n(X).q(Y):-p(Y).";
    let ordinary = admit(program, FormulaLimits::default()).unwrap();
    let observed = admit(
        &format!("{program}#minimize{{Y@7:q(Y)}}."),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(ordinary.atoms(), observed.atoms());
    assert_eq!(ordinary.theory().nodes(), observed.theory().nodes());
    assert_eq!(ordinary.theory().roots(), observed.theory().roots());
    assert_eq!(ordinary.formula_origins(), observed.formula_origins());
}

#[test]
fn unsupported_observer_paths_keep_located_refusals() {
    for source in [
        "n(N):-N=#count{}.p(X):-n(X),X=0.#minimize{X:p(X)}.",
        "n(N):-N=#count{}.p(X):-n(X),d(X).d(0).#minimize{X:p(X)}.",
        "n(N):-N=#count{}.p(0):-n(0).#minimize{X:p(X)}.",
        "n(N,N):-N=#count{}.p(X):-n(X,X).#minimize{X:p(X)}.",
        "n(N):-N=#count{}.p(X,X):-n(X).#minimize{X,Y:p(X,Y)}.",
        "n(N):-N=#count{}.p(f(X)):-n(X).#minimize{1,X:p(X)}.",
        "n(N):-N=#count{}.p(X):-n(X).p(3).#minimize{X:p(X)}.",
        "n(N):-N=#count{}.p(X):-n(X).{p(3)}.#minimize{X:p(X)}.",
        "n(N):-N=#count{}.p(X):-n(X).p(X):-p(X).#minimize{X:p(X)}.",
        "n(N):-N=#count{1:p(X)}.p(X):-n(X).#minimize{X:p(X)}.",
        "n(N):-N=#count{}.p(X):-n(X).q(X):-p(X).#minimize{1:q(0)}.",
        "n(N):-N=#count{}.p(X):-n(X).q(X):-p(X).#minimize{X:q(X),X!=0}.",
        "n(N):-N=#count{}.p(X):-n(X).q(X):-p(X).#minimize{X:q(X),q(X)}.",
        "n(N):-N=#count{}.p(X):-n(X).m(M):-M=#sum{X:p(X),X>0}.#minimize{M:m(M)}.",
        "a.n(N):-N=#count{1:a}.p(X):-n(X).#minimize{1@7:p(0)}.",
        "d(k).a.n(G,N):-d(G),N=#count{1:a}.p(Y,X):-n(X,Y).#minimize{1@7,K:p(0,K)}.",
    ] {
        let error = admit(source, FormulaLimits::default()).unwrap_err();
        assert!(!error.diagnostics().is_empty(), "{source}: located refusal");
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                    feature: ProfileFeature::ObjectiveAggregateDependency,
                    ..
                }))
            ),
            "{source}: {error}"
        );
    }
}

#[test]
fn unqualified_extrema_do_not_claim_numeric_presence() {
    // The first seven unchanged originals now have a flat-carrier certificate.
    let cases: Vec<_> = cases(EXTREMA_REFUSALS).into_iter().skip(7).collect();
    assert_eq!(cases.len(), 3);
    for case in cases {
        let error = admit(&case.source, FormulaLimits::default()).unwrap_err();
        let FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
            feature,
            location,
        })) = error
        else {
            panic!(
                "{}: expected the objective presence refusal: {error}",
                case.name
            );
        };
        assert_eq!(feature, ProfileFeature::ObjectiveAggregateDependency);
        assert!(
            !location.span.is_empty(),
            "{}: producer location",
            case.name
        );
    }
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
    .expect("structural preparation is not completed objective grounding");
    let error = prepared.ground().unwrap_err();
    assert!(matches!(
        error,
        FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
            feature: ProfileFeature::ObjectiveAggregateDependency,
            ..
        }))
    ));
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
    let reference = admit(source, FormulaLimits::default()).unwrap();
    for resource in [
        Resource::Work,
        Resource::Substitutions,
        Resource::SupportRounds,
        Resource::AnalysisNodes,
        Resource::AnalysisEdges,
    ] {
        let mut lower = 0;
        let mut upper = 10_000;
        assert!(admit(source, resource.limits(upper)).is_ok());
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if admit(source, resource.limits(middle)).is_ok() {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(lower > 0);
        let exact = admit(source, resource.limits(lower)).unwrap();
        assert_eq!(exhaustive(&exact), exhaustive(&reference));
        let error = admit(source, resource.limits(lower - 1)).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Limit { resource: actual, observed, limit, .. }
                if actual == resource.kind() && observed > limit),
            "{error}"
        );
        assert!(!error.diagnostics().is_empty());
        assert_eq!(
            exhaustive(&admit(source, resource.limits(lower)).unwrap()),
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
