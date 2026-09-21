//! Complete native model and objective records for bounded source campaigns.
use std::collections::BTreeSet;

use serde_json::Value as Json;
use zetesis_core::{Atom, Model, Sign, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    admit_formula,
};

pub type Records = BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)>;
pub(super) fn atoms(values: &Json) -> BTreeSet<String> {
    values
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| atom.as_str().unwrap().into())
        .collect()
}
pub(super) fn costs(values: &Json) -> Option<Vec<i64>> {
    values
        .as_array()
        .map(|values| values.iter().map(|v| v.as_i64().unwrap()).collect())
}
pub(super) fn canonical(atom: &Atom) -> String {
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    let name = format!("{sign}{}", atom.predicate().name());
    let arguments: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value {
            Value::Infimum => "#inf".into(),
            Value::Supremum => "#sup".into(),
            Value::Structured(value) => value.to_string(),
            Value::Number(value) => value.to_string(),
            Value::Symbol(value) => value.clone(),
            Value::String(value) => serde_json::to_string(value).unwrap(),
        })
        .collect();
    if arguments.is_empty() {
        name
    } else {
        format!("{name}({})", arguments.join(","))
    }
}
pub fn admit(source: &str, limits: &FormulaLimits) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        *limits,
    )
}
pub fn exhaustive(input: &AdmittedFormula) -> Records {
    let count = input.atoms().len();
    assert!(count <= 12, "small independent subset enumeration");
    let cancellation = Cancellation::default();
    let mut records = Records::new();
    for bits in 0..(1_usize << count) {
        let candidate = Interpretation::new(
            input.theory(),
            (0..count).filter(|&atom| bits & (1 << atom) != 0),
        )
        .unwrap();
        if !check(input.theory(), &candidate, Limits::default(), &cancellation)
            .unwrap()
            .accepted()
        {
            continue;
        }
        let atoms: Vec<_> = candidate
            .atoms()
            .map(|atom| input.atoms()[atom].clone())
            .collect();
        let evaluation = zetesis_objective::evaluate(
            input.objectives(),
            &Model::new(atoms.iter().cloned()),
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .unwrap();
        let score = evaluation.score();
        let costs = score
            .is_present()
            .then(|| score.costs().iter().map(|&(_, value)| value).collect());
        assert!(records.insert((atoms.iter().map(canonical).collect(), costs)));
    }
    records
}
