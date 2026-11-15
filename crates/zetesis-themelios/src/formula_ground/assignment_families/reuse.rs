//! A completed occurrence replay must actually reach the native consumer.
//! The shared carrier test helper checks exact emission and all small M/J pairs.

fn same_truth(source: &str) {
    let used = super::carrier_reuse::same_truth(source);
    assert!(
        used.consumed > 0,
        "must complete fresh native occurrence replay: {source}"
    );
}

#[test]
fn retained_family_preserves_duplicate_full_tuple_conditions() {
    for source in [
        "{a;b}. ok :- N=#count{1:a;1:b;2:a}, N=1.",
        "{a;b}. ok :- N=#sum{2,one:a;2,two:b;2,one:b;-1,three:a}, N=2.",
        "{p}. ok :- N=#sum{2:p,not p;3:p}, N=3.",
    ] {
        same_truth(source);
    }
}

#[test]
fn retained_family_preserves_required_and_signed_contributions() {
    for source in [
        "{a}. ok :- N=#sum{2;2:a;3:a;name:a}, N=2.",
        "{a;b}. ok :- N=#sum+{-2:a;0:b;2:a;3:b}, N=2.",
        "{a;b}. ok :- N=#sum{-3:a;2:b}, N=-1.",
    ] {
        same_truth(source);
    }
}

#[test]
fn retained_family_preserves_empty_and_typed_extrema() {
    for source in [
        "{a}. ok :- N=#min{4:a}, N=4.",
        "{a}. ok :- N=#max{4:a}, N=4.",
        "{a}. ok :- N=#min{4;2:a;7:a}, N=2.",
        "{-p(f(a));p(a)}. ok :- N=#max{f(a): -p(f(a));a:not p(a)}, N=f(a).",
    ] {
        same_truth(source);
    }
}

#[test]
fn retained_family_uses_every_read_input_and_the_current_frame() {
    for source in [
        "d(0;1). {a}. ok :- d(X), d(Y), N=#count{1:a}, N=1.",
        "d(0;1). p(1). {a}. ok :- d(X), d(Y), N=#count{Z:p(Z),Z=X,not a}, N=1.",
        "d(f(0);f(1)). {a}. ok :- d(f(X)), N=#count{Y:d(f(Y)),Y=X,a}, N=1.",
        "{a}. ok :- A=#count{1:a}, B=#sum{A:a;2,A:not a}, B=A.",
    ] {
        same_truth(source);
    }
}

#[test]
fn missing_local_plan_certificate_keeps_exact_fallback() {
    let source = "d(0;1). {a}. ok(X,N) :- d(X), N=#count{1:a}.";
    let used = crate::formula_support::testing::without_local_plans(|| {
        super::carrier_reuse::same_truth(source)
    });
    // The common helper requires actual produced and borrowed carriers and
    // checks exact emission plus every small original and frozen M/J pair.
    assert_eq!(
        used.consumed, 0,
        "owned plans give no retained replay certificate"
    );
}
