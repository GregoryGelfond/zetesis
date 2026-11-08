//! The accurate raw interpretation name preserves the original set API.

use zetesis_core::{Atom, Interpretation, Model, Predicate, Sign, Value};

#[test]
fn raw_interpretation_preserves_model_compatibility() {
    let atom = Atom::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap();
    let interpretation = Interpretation::new([atom.clone(), atom.clone()]).unwrap();
    let legacy: Model = interpretation;
    assert_eq!(legacy.atoms().len(), 1);
    assert!(legacy.contains(&atom));
    let returned: Interpretation = legacy;
    assert_eq!(returned, Model::new([atom]).unwrap());
}

#[test]
fn an_atoms_contrary_flips_only_its_sign() {
    let positive = Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(3)]).unwrap();
    let negative = Atom::new(
        Predicate::with_sign("p", 1, Sign::Negative).unwrap(),
        vec![Value::Number(3)],
    )
    .unwrap();
    assert_eq!(positive.contrary(), negative);
    assert_eq!(negative.contrary(), positive);
}
