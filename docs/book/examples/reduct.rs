//! Test a disjunction's frozen reduct against proper subsets of its candidate.

// ANCHOR: example
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, FrozenReduct, Interpretation, Limits, Node, Theory, Verdict, check, models,
    models_reduct,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theory = Theory::new(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
        vec![2],
        AdmissionLimits::default(),
    )?;
    let candidate = Interpretation::new(&theory, [0, 1])?;
    let cancellation = Cancellation::default();
    let limits = Limits::default();

    assert!(models(&theory, &candidate, limits, &cancellation)?);
    let decision = check(&theory, &candidate, limits, &cancellation)?;
    let Verdict::NonMinimal { witness } = decision.verdict() else {
        panic!("the disjunction has a singleton countermodel");
    };
    assert_eq!(witness.atoms().count(), 1);
    assert!(models_reduct(
        &theory,
        &candidate,
        witness,
        limits,
        &cancellation
    )?);
    assert!(!decision.accepted());

    let reduct = FrozenReduct::new(&candidate, limits, &cancellation)?;
    for atom in [0, 1] {
        let singleton = Interpretation::new(&theory, [atom])?;
        assert!(reduct.is_satisfied_by(&singleton, limits, &cancellation)?);
    }
    let empty = Interpretation::new(&theory, [])?;
    assert!(!reduct.is_satisfied_by(&empty, limits, &cancellation)?);
    Ok(())
}
// ANCHOR_END: example
