//! The accurate raw interpretation name preserves the original set API.

use zetesis_core::{Atom, Interpretation, Model, Predicate};

#[test]
fn raw_interpretation_preserves_model_compatibility() {
    let atom = Atom::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap();
    let interpretation = Interpretation::new([atom.clone(), atom.clone()]);
    let legacy: Model = interpretation;
    assert_eq!(legacy.atoms().len(), 1);
    assert!(legacy.contains(&atom));
    let returned: Interpretation = legacy;
    assert_eq!(returned, Model::new([atom]));
}
