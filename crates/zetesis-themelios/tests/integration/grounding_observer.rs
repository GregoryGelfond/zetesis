//! Optional materialization observation preserves admission and failure behavior.

use crate::support::grounding_observers::Observer;
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula,
    admit_formula_with_grounding_observer,
};

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
    assert_eq!(
        (measured.theory().nodes(), measured.theory().operands()),
        (plain.theory().nodes(), plain.theory().operands())
    );
    assert_eq!(measured.theory().roots(), plain.theory().roots());
    assert_eq!(*observer.0.borrow(), [true, false]);

    for (source, enters) in [
        ("a(.", false),
        ("p(X).", false),
        ("v(2147483647).0<=#min{X:a:v(X)}.", true),
    ] {
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
