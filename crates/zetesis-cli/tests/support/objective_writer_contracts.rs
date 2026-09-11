//! Diagnostic transport failure must not conceal a refused candidate restriction.

use std::collections::BTreeSet;
use std::io::{self, Write};

use clap::Parser;
use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_ferraris::Interpretation;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

use super::Bounds;
use crate::countermodel::Input;
use crate::presentation::Diagnostics;
use crate::test_writer::BoundedWriter;
use crate::{ColorMode, Options, RunError};

fn admitted(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

fn attempt(foreign: bool, sink: &mut impl Write) -> (Result<(), RunError>, StableModels) {
    let planned = admitted("{a;b}. #minimize{1,a:a;1,b:b}.");
    let different = foreign.then(|| admitted("{x;y}."));
    let original = different.as_ref().unwrap_or(&planned);
    let options = Options::try_parse_from(["zetesis"]).unwrap();
    let candidate = Interpretation::new(planned.theory(), [0]).unwrap();
    assert!(
        zetesis_ferraris::check(
            planned.theory(),
            &candidate,
            zetesis_ferraris::Limits::default(),
            &Control::default()
        )
        .unwrap()
        .accepted()
    );
    let model = Model::new(candidate.atoms().map(|id| planned.atoms()[id].clone()));
    let score = zetesis_objective::evaluate(
        planned.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &Control::default(),
    )
    .unwrap();
    let mut bounds = Bounds::new(
        Input {
            theory: planned.theory(),
            atoms: planned.atoms(),
            gate_atoms: 0,
            objectives: planned.objectives(),
            observations: planned.metadata().observations(),
        },
        &(&options).into(),
        &mut crate::execution_observation::Ignore,
        &Control::default(),
    )
    .unwrap();
    assert!(bounds.plan.is_some());
    let mut limits = Limits::default();
    if !foreign {
        limits.admission.max_variables = original.atoms().len();
    }
    let mut models = StableModels::new(original.theory(), limits, Control::default()).unwrap();
    let result = bounds.improve(
        score.score(),
        &mut models,
        &(&options).into(),
        &mut Diagnostics::new(sink, ColorMode::Never),
        &Control::default(),
    );
    assert_eq!(models.statistics().candidate_restrictions, 0);
    assert!(models.theory().same_instance(original.theory()));
    assert!(
        !models.exhausted(),
        "a refused bound did not exhaust original search"
    );
    (result, models)
}

fn all_original_models(mut models: StableModels) {
    let actual: BTreeSet<Vec<usize>> = models
        .by_ref()
        .map(|result| result.unwrap().atoms().collect())
        .collect();
    assert!(models.exhausted());
    assert_eq!(
        actual,
        BTreeSet::from([vec![], vec![0], vec![1], vec![0, 1]])
    );
}

fn check_refusal(foreign: bool, cause: &str) {
    let mut reference = Vec::new();
    let (result, models) = attempt(foreign, &mut reference);
    result.unwrap();
    all_original_models(models);
    let text = std::str::from_utf8(&reference).unwrap();
    assert!(text.starts_with("Objective pruning stopped:"));
    assert!(text.contains(cause), "{text}");
    assert!(text.ends_with("; exact search continues\n"));
    for capacity in [0, "Objective pruning stopped:".len(), reference.len() - 1] {
        let mut sink = BoundedWriter::new(capacity);
        let (result, models) = attempt(foreign, &mut sink);
        let RunError::Output(error) = result.unwrap_err() else {
            panic!("original output failure must remain typed")
        };
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(error.to_string(), "diagnostic sink closed");
        assert_eq!(sink.bytes(), &reference[..capacity]);
        all_original_models(models);
    }
}

#[test]
fn foreign_theory_diagnostic_failure_preserves_the_original_search() {
    check_refusal(true, "original theory mismatch");
}

#[test]
fn transactional_restriction_refusal_propagates_the_real_writer_error() {
    check_refusal(false, "Variables");
}
