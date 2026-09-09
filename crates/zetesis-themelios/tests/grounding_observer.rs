//! Optional materialization observation preserves admission and failure behavior.

use std::cell::RefCell;
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, GroundingObserver, admit_formula,
    admit_formula_with_grounding_observer,
};

#[derive(Default)]
struct Observer(RefCell<Vec<bool>>);
impl GroundingObserver for Observer {
    fn enter(&self) {
        self.0.borrow_mut().push(true);
    }
    fn exit(&self) {
        self.0.borrow_mut().push(false);
    }
}

#[test]
fn exact_grounding_boundary_preserves_success_and_retains_failed_attempts() {
    let source = "1#count{X:p(X):X=1..3}2.";
    let observer = Observer::default();
    let measured = admit_formula_with_grounding_observer(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
        Some(&observer),
    )
    .unwrap();
    let plain = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(measured.atoms(), plain.atoms());
    assert_eq!(measured.theory().nodes(), plain.theory().nodes());
    assert_eq!(measured.theory().roots(), plain.theory().roots());
    assert_eq!(*observer.0.borrow(), [true, false]);

    for (source, enters) in [("a(.", false), ("p(X).", false), ("1#min{1:a;1:b}1.", true)] {
        let observer = Observer::default();
        let result = admit_formula_with_grounding_observer(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
            Some(&observer),
        );
        assert!(result.is_err(), "{source}");
        let plain = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert_eq!(result.unwrap_err().to_string(), plain.to_string());
        assert_eq!(
            *observer.0.borrow(),
            if enters { vec![true, false] } else { vec![] }
        );
    }
}
