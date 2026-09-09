//! Inspect the analysis owner before constructing and solving a finite theory.

// ANCHOR: example
use zetesis_cli::{Backend, PreparedInput, SolveConfig, WorldView, WorldViewLimits};
use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionOptions, AnalysisBasis, ExpansionLimits, FormulaFailure, FormulaLimits,
    FormulaResource, prepare_formula,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Two written Boolean choices contribute twice. Two occurrences of one
    // explicit tuple contribute once. Neither program introduces an atom.
    let cases = [
        (
            "2{#true;#true}2.",
            AnalysisBasis::DependencyProjection,
            1,
            1,
        ),
        (
            "2#count{1:#true;1:#true}2.",
            AnalysisBasis::NormalizedProgram,
            1,
            0,
        ),
        ("", AnalysisBasis::NormalizedProgram, 0, 1),
        (":-.", AnalysisBasis::NormalizedProgram, 1, 0),
    ];
    for (source, basis, statement_count, answer_count) in cases {
        let prepared = prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )?;
        assert_eq!(prepared.analysis_basis(), basis);
        assert_eq!(
            prepared.analyzed_program().statements().count(),
            statement_count
        );
        // This verdict describes the inspected analysis input. In the first
        // case it is not a source-semantic safety or class certificate.
        assert!(prepared.source_analysis().safety().is_safe());

        let admitted = prepared.ground()?;
        assert_eq!(admitted.analysis_basis(), basis);
        assert!(admitted.atoms().is_empty());
        assert_eq!(
            admitted.formula_origins().len(),
            admitted.theory().roots().len()
        );
        if statement_count > 0 {
            assert!(admitted.formula_origins().iter().flatten().next().is_some());
        }
        assert!(admitted.formula_origins().iter().flatten().all(|origin| {
            origin.source == admitted.source().id() && admitted.source().slice(origin.span).is_ok()
        }));
        let family = WorldView::collect(
            PreparedInput::formula(&admitted),
            SolveConfig {
                backend: Backend::Cpu,
                models: 0,
                ..SolveConfig::default()
            },
            WorldViewLimits::default(),
            Control::default(),
        )?;
        assert_eq!(family.len(), answer_count);
        assert!(
            family
                .answer_sets()
                .iter()
                .all(|answer| { answer.interpretation().atoms().is_empty() })
        );
        assert_eq!(family.outcome().unsatisfiable(), answer_count == 0);
    }

    // A resource refusal is located evidence about preparation, not UNSAT.
    let options = AdmissionOptions::default();
    let error = prepare_formula(
        "p.".into(),
        options,
        ExpansionLimits::default(),
        FormulaLimits {
            max_analysis_nodes: 0,
            ..FormulaLimits::default()
        },
    )
    .expect_err("the nonempty analysis exceeds a zero-node ceiling");
    let FormulaFailure::Limit {
        resource,
        limit,
        observed,
        location,
    } = error
    else {
        panic!("expected an analysis resource refusal");
    };
    assert_eq!(resource, FormulaResource::AnalysisNodes);
    assert_eq!(limit, 0);
    assert!(observed > limit);
    assert_eq!(location.source, options.source_id);
    Ok(())
}
// ANCHOR_END: example
