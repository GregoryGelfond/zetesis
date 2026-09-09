// ANCHOR: example
extern crate zetesis_cpu;
extern crate zetesis_ferraris;

use zetesis_cpu::Control;
use zetesis_ferraris::{
    AdmissionLimits, Interpretation, Limits, Node, Theory, Verdict, check, models, models_reduct,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theory = Theory::new(
        2,
        vec![Node::Atom(0), Node::Atom(1), Node::Or(0, 1)],
        vec![2],
        AdmissionLimits::default(),
    )?;
    let candidate = Interpretation::new(&theory, [0, 1])?;
    let control = Control::default();
    let limits = Limits::default();

    assert!(models(&theory, &candidate, limits, &control)?);
    let decision = check(&theory, &candidate, limits, &control)?;
    let Verdict::NonMinimal { witness } = decision.verdict() else {
        panic!("the disjunction has a singleton countermodel");
    };
    assert_eq!(witness.atoms().count(), 1);
    assert!(models_reduct(
        &theory, &candidate, witness, limits, &control
    )?);
    assert!(!decision.accepted());
    Ok(())
}
// ANCHOR_END: example
