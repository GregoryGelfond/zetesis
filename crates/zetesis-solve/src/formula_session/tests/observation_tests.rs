//! Admitted graph populations count logical operands independently of storage.

use super::{FormulaSession, Input};
use crate::execution_observation::ExecutionSink;
use crate::formula_execution::Execution;
use crate::phase_timing::Recorder;
use crate::{AnswerSelection, Backend, ExecutionObservation, SolveConfig, SolveError};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node, OperandSpan, Theory};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

#[derive(Default)]
struct Population(Option<(usize, usize, usize, usize)>);

impl ExecutionSink for Population {
    fn record(&mut self, event: ExecutionObservation<'_>) -> Result<(), SolveError> {
        if let ExecutionObservation::Formula {
            atoms,
            nodes,
            operands,
            roots,
            ..
        } = event
        {
            assert!(self.0.replace((atoms, nodes, operands, roots)).is_none());
        }
        Ok(())
    }
}

#[test]
fn formula_observation_counts_logical_operands() {
    let owner = admit_formula(
        "{a;b;c}.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    // Four wide occurrences include a repeated child; the pair and implication
    // add two each without occupying arena cells. The complete graph has E = 8.
    let theory = Theory::new(
        3,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::or_span(OperandSpan {
                    start: 0,
                    length: 4,
                }),
                Node::and_pair([0, 1]),
                Node::implies(4, 3),
            ],
            vec![0, 1, 2, 1],
        )
        .unwrap(),
        vec![5],
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(theory.operands().len(), 4);
    let config = SolveConfig {
        backend: Backend::Cpu,
        workers: std::num::NonZeroUsize::MIN,
        ..SolveConfig::default()
    };
    let mut observed = Population::default();
    let session = FormulaSession::with_selection(
        Input {
            theory: &theory,
            atoms: owner.atom_catalog(),
            objectives: owner.objectives(),
            required_choices: None,
            gate_atoms: 0,
            keyed_constraints: 0,
            key_analysis: zetesis_themelios::KeyAnalysis::Complete,
            certificate_order: zetesis_sat::CertificateOrder::TightFirst,
        },
        Execution::Cpu,
        &config,
        &mut observed,
        &Cancellation::default(),
        &Recorder::new(false),
        AnswerSelection::All,
    );
    assert!(!session.finished());
    assert_eq!(observed.0, Some((3, 6, 8, 1)));
}
