//! Bound-column probes restrict work without replacing full relational matching.

use std::cell::Cell;
use std::fmt::Write;

use themelios_base::span::Location;
use themelios_program::term::EvalError;

use zetesis_core::Value;
use zetesis_cpu::Control;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    AdmissionOptions, ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits,
    FormulaResource, GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork,
    admit_formula, admit_formula_with_grounding_observer,
};

const JOIN_RULE: &str = "r(X,Z):-p(X,Y),q(Y,Z).";

fn selective_source() -> String {
    let mut source = String::new();
    for value in 0..200 {
        write!(source, "p({value},{value}).q({value},{}).", value + 1).expect("write to String");
    }
    source.push_str(JOIN_RULE);
    source
}

#[derive(Default)]
struct JoinVisits {
    active: Cell<bool>,
    support: Cell<u64>,
    instantiation: Cell<u64>,
}

impl GroundingObserver for JoinVisits {
    fn enter(&self) {
        assert!(!self.active.replace(true));
    }

    fn exit(&self) {
        assert!(self.active.replace(false));
    }

    fn details_enabled(&self) -> bool {
        assert!(self.active.get());
        true
    }

    fn phase_exit(
        &self,
        phase: GroundingPhase,
        _: Option<Location>,
        outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        assert_eq!(outcome, GroundingOutcome::Completed);
        let target = match phase {
            GroundingPhase::SupportCompletion => &self.support,
            GroundingPhase::RuleInstantiation => &self.instantiation,
            _ => return,
        };
        target.set(
            target
                .get()
                .checked_add(work.join_rows.expect("finite visits"))
                .unwrap(),
        );
    }
}

#[test]
fn shared_arguments_select_only_matching_rows() {
    let visits = JoinVisits::default();
    let input = admit_formula_with_grounding_observer(
        selective_source(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
        Some(&visits),
    )
    .expect("selective lookup avoids unrelated relation pairs");
    // Whole-grounding work now includes dictionary construction and rebuilds.
    // The direct visit count detects replacing the index with a 200 × 200 scan.
    assert_eq!(visits.instantiation.get(), 400);
    assert!(visits.support.get() < 200 * 200);
    println!(
        "support_rows={} instantiation_rows={}",
        visits.support.get(),
        visits.instantiation.get()
    );
    let mut search = StableModels::new(input.theory(), Limits::default(), Control::default())
        .expect("finite theory");
    let model = search.next().expect("one model").expect("verified model");
    let joined: Vec<_> = model
        .atoms()
        .map(|index| &input.atoms()[index])
        .filter(|atom| atom.predicate().name() == "r")
        .collect();
    assert_eq!(joined.len(), 200);
    for atom in joined {
        let [Value::Number(left), Value::Number(right)] = atom.values() else {
            panic!("numeric relation")
        };
        assert_eq!(*right, *left + 1);
    }
    assert!(search.next().is_none());
    assert!(search.exhausted());
}

#[test]
fn selective_join_work_limit_is_inclusive() {
    let source = selective_source();
    let compile = |max_work| {
        admit_formula(
            source.clone(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_work,
                ..FormulaLimits::default()
            },
        )
    };
    let (mut low, mut high) = (0, FormulaLimits::default().max_work);
    assert!(compile(high).is_ok());
    while low < high {
        let middle = low + (high - low) / 2;
        match compile(middle) {
            Ok(_) => high = middle,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                observed,
                limit,
                ..
            }) => {
                assert_eq!(limit, u128::from(middle));
                assert!(observed > limit);
                low = middle + 1;
            }
            other => panic!("unexpected admission: {other:?}"),
        }
    }
    assert!(low > 0);
    assert!(compile(low).is_ok());
    let Err(FormulaFailure::Limit {
        resource: FormulaResource::Work,
        observed,
        limit,
        location,
    }) = compile(low - 1)
    else {
        panic!("the preceding work ceiling must refuse");
    };
    assert_eq!(limit, u128::from(low - 1));
    assert_eq!(observed, u128::from(low));
    assert_eq!(location.source, AdmissionOptions::default().source_id);
    assert_eq!(
        &source[location.span.start().get() as usize..location.span.end().get() as usize],
        JOIN_RULE,
    );
    println!("complete_work={low}");
}

#[test]
fn indexed_candidates_still_validate_repeated_variables_and_every_constant() {
    let input = admit_formula(
        "p(1;2).q(1,2,a).q(1,1,b).q(2,2,b).q(2,2,a).r(X):-p(X),q(X,X,a).".to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("full row match");
    let mut search = StableModels::new(input.theory(), Limits::default(), Control::default())
        .expect("finite theory");
    let model = search.next().expect("one model").expect("verified model");
    let joined: Vec<_> = model
        .atoms()
        .map(|index| &input.atoms()[index])
        .filter(|atom| atom.predicate().name() == "r")
        .collect();
    assert_eq!(joined.len(), 1);
    assert_eq!(joined[0].values(), &[Value::Number(2)]);
    assert!(search.next().is_none());
    assert!(search.exhausted());
}

#[test]
fn retained_index_entries_have_an_independent_inclusive_ceiling() {
    for maximum in [0, 1, 2] {
        let result = admit_formula(
            "p(1,2).".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_support_index_entries: maximum,
                ..FormulaLimits::default()
            },
        );
        if maximum == 2 {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::SupportIndexEntries,
                    observed: 2,
                    ..
                })
            ));
        }
    }
    assert!(
        admit_formula(
            "p.".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                max_support_index_entries: 0,
                ..FormulaLimits::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn ready_filters_prune_before_later_relations_without_reading_generated_slots() {
    let input = admit_formula(
        "left(1..30).right(1..30).tail(1..30).p(X):-left(X),right(Y),tail(Z),X=Y.".to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_substitutions: 5_000,
            ..FormulaLimits::default()
        },
    )
    .expect("ready equality removes incompatible partial rows before tail expansion");
    let mut search =
        StableModels::new(input.theory(), Limits::default(), Control::default()).expect("theory");
    let model = search.next().expect("one model").expect("verified model");
    assert_eq!(
        model
            .atoms()
            .map(|index| &input.atoms()[index])
            .filter(|atom| atom.predicate().name() == "p")
            .count(),
        30
    );
    assert!(search.next().is_none());
    assert!(search.exhausted());

    let input = admit_formula(
        "d(5).p(Y):-d(X),Y=X+1,Y>5.".to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("generated comparison waits for binding");
    let mut search =
        StableModels::new(input.theory(), Limits::default(), Control::default()).expect("theory");
    let model = search.next().expect("one model").expect("verified model");
    assert!(
        model
            .atoms()
            .map(|index| &input.atoms()[index])
            .any(|atom| atom.predicate().name() == "p" && atom.values() == [Value::Number(6)])
    );
    assert!(search.next().is_none());
    assert!(search.exhausted());
}

#[test]
fn invalid_arithmetic_on_unextendable_prefixes_does_not_refuse_the_source() {
    for source in [
        "a(0;1).b(1;2).p(X):-a(X),b(X),1/X>0.",
        "a(1;2147483647).b(1;2).p(X):-a(X),b(X),X+1>0.",
    ] {
        let input = admit_formula(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .expect("dead relational prefix cannot justify scalar refusal");
        let mut search = StableModels::new(input.theory(), Limits::default(), Control::default())
            .expect("theory");
        let model = search.next().expect("one model").expect("verified model");
        let outputs: Vec<_> = model
            .atoms()
            .map(|index| &input.atoms()[index])
            .filter(|atom| atom.predicate().name() == "p")
            .collect();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].values(), &[Value::Number(1)]);
        assert!(search.next().is_none());
        assert!(search.exhausted());
    }
}

#[test]
fn duplicate_support_heads_do_not_remove_alternative_final_reduct_witnesses() {
    let input = admit_formula(
        "d(1;2).{q(1);q(2)}.p:-d(X),q(X).".to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("support projection preserves full final body disjunction");
    let mut search =
        StableModels::new(input.theory(), Limits::default(), Control::default()).expect("theory");
    let mut count = 0;
    for model in search.by_ref() {
        let model = model.expect("verified model");
        let names: Vec<_> = model
            .atoms()
            .map(|index| input.atoms()[index].predicate().name())
            .collect();
        assert_eq!(names.contains(&"p"), names.contains(&"q"));
        count += 1;
    }
    assert_eq!(count, 4);
    assert!(search.exhausted());
}

#[test]
fn false_filters_preserve_required_body_assignment_errors() {
    let source = "a.b.p(N):-N=#sum{2147483647:a;1:b},1=2.";
    // The empty positive join has one row. The authored body assignment must
    // supply logical scalar N, unlike head-only generated terms that wait for
    // body selection. Clingo's successful {a,b} record remains a difference.
    let error = admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect_err("2147483648 cannot inhabit the required i32 assignment");
    let FormulaFailure::Expansion(ExpansionFailure::Evaluation {
        error: EvalError::Overflow,
        location,
    }) = error
    else {
        panic!("expected located assignment overflow: {error:?}");
    };
    assert_eq!(location.source, themelios_base::source::SourceId::new(0));
    assert_eq!(location.span.start().get(), 4);
    assert_eq!(
        usize::try_from(location.span.end().get()).unwrap(),
        source.len()
    );
}
