//! Source aggregates expose prefix consequences before candidate branching.

mod preservation;

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    Narrower, Narrowing, NarrowingScratch, OriginalSubject, Region, RegionLimits,
};
use zetesis_reference_support::formula;

fn observer(source: &str) -> Option<bool> {
    let program = formula(source);
    // Both guards must reach the native path. Boolean expansion could otherwise
    // satisfy this regression through connective propagation alone.
    assert_eq!(program.theory().parts().thresholds().len(), 2);
    let narrower = Narrower::new(program.theory());
    let mut region = Region::all_open(program.atoms().len());
    let (outcome, _) = narrower
        .narrow_known(
            OriginalSubject::new(program.theory(), None),
            &mut region,
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_ne!(outcome, Narrowing::Refuted);
    let mut observer = None;
    for (atom, value) in program.atoms().iter().enumerate() {
        if value.predicate().name() == "z" {
            observer = Some(region.decision(atom));
        } else {
            assert!(region.is_open(atom), "contributions must remain undecided");
        }
    }
    observer.expect("the source declares z")
}

#[test]
fn false_extended_count_excludes_prefix_count() {
    assert_eq!(
        observer(
            "{a;b;c;d;z}. \
             :- #count{1:a;2:b;3:c;4:d} >= 2. \
             :- z, not #count{1:a;2:b;3:c} >= 2."
        ),
        Some(false)
    );
}

#[test]
fn true_prefix_count_requires_extended_count() {
    assert_eq!(
        observer(
            "{a;b;c;d;z}. \
             :- #count{1:a;2:b;3:c} < 2. \
             :- not z, #count{1:a;2:b;3:c;4:d} >= 2."
        ),
        Some(true)
    );
}
