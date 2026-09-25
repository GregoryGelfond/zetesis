//! Complete native model and objective records for bounded source campaigns.
use std::collections::BTreeSet;

use serde_json::Value as Json;
use zetesis_core::{Model, Sign};
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
pub(super) fn canonical<'a>(atom: impl Into<zetesis_core::catalog::AtomRef<'a>>) -> String {
    let atom = atom.into();
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    let name = format!("{sign}{}", atom.predicate().name());
    let arguments: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value.descriptor() {
            zetesis_core::ValueNodeRef::Infimum => "#inf".into(),
            zetesis_core::ValueNodeRef::Supremum => "#sup".into(),
            zetesis_core::ValueNodeRef::Function { .. }
            | zetesis_core::ValueNodeRef::Tuple { .. } => value.to_string(),
            zetesis_core::ValueNodeRef::Number(value) => value.to_string(),
            zetesis_core::ValueNodeRef::Symbol(value) => value.to_owned(),
            zetesis_core::ValueNodeRef::String(value) => serde_json::to_string(value).unwrap(),
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
            .map(|atom| input.atoms().at(atom).unwrap())
            .collect();
        let evaluated_model =
            Model::from_positions(input.atom_catalog(), candidate.atoms()).unwrap();
        let evaluation = zetesis_objective::evaluate(
            input.objectives(),
            &evaluated_model,
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .unwrap();
        let score = evaluation.score();
        let costs = score
            .is_present()
            .then(|| score.costs().iter().map(|&(_, value)| value).collect());
        assert!(records.insert((atoms.iter().copied().map(canonical).collect(), costs)));
    }
    records
}
