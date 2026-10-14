//! Diagnostic certificates preserve occurrence identity and conservative declines.

use std::cell::{Cell, RefCell};

use themelios_program::program::Relation;
use zetesis_domain::{Range, numeric::Status};
use zetesis_reference_support::prepare;
use zetesis_themelios::{
    DomainLimits, DomainObservation, GroundingObserver, NumericBody, NumericCensus,
    NumericComparison, NumericOperand, ProgramSite,
};

struct Rule {
    occurrence: usize,
    site: ProgramSite,
    body: NumericBody,
    normal_head: bool,
    comparisons: Vec<NumericComparison>,
}
#[derive(Default)]
struct Observation {
    status: Cell<Option<Status>>,
    census: Cell<Option<NumericCensus>>,
    rules: RefCell<Vec<Rule>>,
    disabled: Cell<bool>,
}
impl GroundingObserver for Observation {
    fn enter(&self) {}
    fn exit(&self) {}
    fn domain_analysis(&self, observation: DomainObservation<'_, '_>) {
        let DomainObservation::Analyzed(view) = observation else {
            self.disabled.set(true);
            return;
        };
        self.status.set(Some(view.analysis().status()));
        self.census.set(view.census());
        self.rules.replace(
            view.rules()
                .map(|rule| Rule {
                    occurrence: rule.occurrence(),
                    site: rule.site(),
                    body: rule.body(),
                    normal_head: rule.normal_head(),
                    comparisons: rule.comparisons().collect(),
                })
                .collect(),
        );
    }
}
fn census(source: &str) -> Observation {
    let observer = Observation::default();
    prepare(source)
        .unwrap()
        .with_domain_analysis(Some(DomainLimits::default()))
        .ground_with_observer(Some(&observer))
        .unwrap();
    assert_eq!(observer.status.get(), Some(Status::FixedPoint));
    assert!(observer.census.get().is_some());
    observer
}

#[test]
fn remainder_totality_survives_another_rules_default_negation() {
    let observation =
        census("d(2..12). composite(N):-d(N),d(D),D<N,N\\D=0. prime(N):-d(N),not composite(N).");
    let count = observation.census.get().unwrap();
    assert_eq!(count.rule_occurrences, 2);
    assert_eq!(count.eligible_rules, 2);
    assert_eq!(count.integer_queries, 4);
    assert_eq!(count.integer_certificates, 4);
    assert_eq!(count.integer_declines, 0);
    assert_eq!(count.total_bodies, 2);
    assert_eq!(count.normal_head_total_bodies, 2);
    let rules = observation.rules.borrow();
    let composite = rules
        .iter()
        .find(|rule| rule.comparisons.len() == 2)
        .unwrap();
    assert_eq!(composite.body, NumericBody::Total);
    assert!(composite.normal_head);
    let remainder = composite
        .comparisons
        .iter()
        .find(|value| value.relation == Relation::Eq)
        .unwrap();
    assert_eq!(
        remainder.left,
        NumericOperand::Integer(Range::new(0, 11).unwrap())
    );
    assert_eq!(remainder.right, NumericOperand::Integer(Range::point(0)));
    assert_eq!(remainder.excluding, Some(true));
    assert_eq!(remainder.separated, None);
}

#[test]
fn produced_guards_do_not_certify_zero_divisors() {
    let observation = census("q(0..2). p(X):-q(X),X>0,5\\X=0.");
    let rules = observation.rules.borrow();
    let rule = &rules[0];
    assert_eq!(rule.body, NumericBody::Uncertified);
    let remainder = rule
        .comparisons
        .iter()
        .find(|value| value.relation == Relation::Eq)
        .unwrap();
    assert_eq!(remainder.left, NumericOperand::Uncertified);
    assert_eq!(remainder.right, NumericOperand::Integer(Range::point(0)));
    assert_eq!(remainder.separated, None);
    assert_eq!(observation.census.get().unwrap().integer_declines, 1);
}

#[test]
fn rich_producers_keep_mixed_leaves_nonnumeric() {
    let observation = census("q(1). {q(a)}. p(X):-q(X),X>0.");
    let count = observation.census.get().unwrap();
    assert_eq!(count.rule_occurrences, 2);
    assert_eq!(count.unsupported_rules, 1);
    assert!(count.mixed_positions > 0);
    let rules = observation.rules.borrow();
    let rule = rules
        .iter()
        .find(|rule| !rule.comparisons.is_empty())
        .unwrap();
    // Bound mixed leaves are total terms, without becoming integer expressions.
    assert_eq!(rule.body, NumericBody::Total);
    assert_eq!(rule.comparisons[0].left, NumericOperand::Leaf);
    assert_eq!(rule.comparisons[0].separated, None);
    assert_eq!(rule.comparisons[0].excluding, Some(true));
}

#[test]
fn correlated_subtraction_does_not_hide_undefined_children() {
    let observation = census("q(0..2). p(X):-q(X),X>0,(1\\X)-(1\\X)=0.");
    let rules = observation.rules.borrow();
    assert_eq!(rules[0].body, NumericBody::Uncertified);
    let equality = rules[0]
        .comparisons
        .iter()
        .find(|value| value.relation == Relation::Eq)
        .unwrap();
    assert_eq!(equality.left, NumericOperand::Uncertified);
    assert_eq!(equality.separated, None);
}

#[test]
fn expanded_siblings_keep_distinct_occurrence_coordinates() {
    let observation = census("d(1..3). p(1;2):-d(X),X<3.");
    let rules = observation.rules.borrow();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].site, rules[1].site);
    assert_eq!((rules[0].occurrence, rules[1].occurrence), (0, 1));
    assert!(rules.iter().all(|rule| rule.body == NumericBody::Total));
    assert!(rules.iter().all(|rule| rule.comparisons.len() == 1));
    assert_eq!(observation.census.get().unwrap().integer_queries, 4);
}

#[test]
fn generated_heads_do_not_acquire_whole_body_authority() {
    let observation = census("q(1..3). p(X+1):-q(X),X<2.");
    let rules = observation.rules.borrow();
    assert_eq!(rules[0].body, NumericBody::Unsupported);
    assert!(
        rules[0]
            .comparisons
            .iter()
            .all(|value| value.excluding.is_none())
    );
    let count = observation.census.get().unwrap();
    assert_eq!(count.unsupported_rules, 1);
    assert_eq!(count.total_bodies, 0);
    assert!(count.integer_certificates > 0);
}

#[test]
fn an_empty_binding_domain_is_not_a_total_family_witness() {
    let observation = census("p(X):-absent(X),X+1>0.");
    let count = observation.census.get().unwrap();
    assert_eq!(count.empty_rules, 1);
    assert!(count.empty_positions > 0);
    assert_eq!(count.total_bodies, 0);
    assert_eq!(observation.rules.borrow()[0].body, NumericBody::Empty);
}

#[test]
fn separated_comparisons_report_all_six_relations_without_pruning() {
    let observation = census("q(1..2). r(5..6). p(X,Y):-q(X),r(Y),X=Y,X!=Y,X<Y,X<=Y,X>Y,X>=Y.");
    let rules = observation.rules.borrow();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].body, NumericBody::Total);
    assert_eq!(rules[0].comparisons.len(), 6);
    for comparison in &rules[0].comparisons {
        let expected = matches!(
            comparison.relation,
            Relation::Neq | Relation::Lt | Relation::Le
        );
        assert_eq!(comparison.separated, Some(expected));
        assert_eq!(comparison.excluding, Some(true));
    }
    let count = observation.census.get().unwrap();
    assert_eq!((count.separated_true, count.separated_false), (3, 3));
    assert_eq!((count.excluding_true, count.excluding_false), (3, 3));
}

#[test]
fn stopped_and_disabled_censuses_are_unavailable() {
    for domains in [
        None,
        Some(DomainLimits {
            max_work: 0,
            ..DomainLimits::default()
        }),
    ] {
        let observer = Observation::default();
        prepare("p(1). q(X):-p(X),X>0.")
            .unwrap()
            .with_domain_analysis(domains)
            .ground_with_observer(Some(&observer))
            .unwrap();
        assert_eq!(observer.census.get(), None);
        assert!(observer.rules.borrow().is_empty());
        if domains.is_some() {
            assert!(matches!(observer.status.get(), Some(Status::Stopped(_))));
        } else {
            assert!(observer.disabled.get());
        }
    }
    let empty = census("");
    assert_eq!(empty.census.get(), Some(NumericCensus::default()));
}

#[test]
fn local_choice_frames_do_not_shift_the_normal_rule_mapping() {
    // Canonical heads put Choice before Falsum, while an atomic head sorts before
    // Choice. The constraint therefore follows two local neutral rule frames:
    // its occurrence is 1, but its returned neutral RuleId must name frame 2.
    let observation = census("d(1..2). e(100..101). {a(X):d(X);b(Y):e(Y)}. :-d(X),X<2.");
    let rules = observation.rules.borrow();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].occurrence, 0);
    assert_eq!(rules[0].body, NumericBody::Unsupported);
    assert!(rules[0].comparisons.is_empty());
    assert_ne!(rules[0].site.statement_id(), rules[1].site.statement_id());
    assert_eq!(rules[1].occurrence, 1);
    assert!(!rules[1].normal_head);
    assert_eq!(rules[1].body, NumericBody::Total);
    assert_eq!(rules[1].comparisons.len(), 1);
    assert_eq!(
        rules[1].comparisons[0].left,
        NumericOperand::Integer(Range::new(1, 2).unwrap())
    );
    assert_eq!(rules[1].comparisons[0].separated, None);
}
