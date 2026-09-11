//! Score and display completed answers without changing their identity.

// ANCHOR: example
use zetesis_cpu::Control;
use zetesis_solve::{
    Backend, Completion, Grounder, Oracle, PreparedInput, Session, SolveConfig, WorldViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula, observation,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
        site(east,3). site(west,1).
        1 { use(S):site(S,_) } 1.
        #minimize { C@1,S:use(S),site(S,C); 2@1,remote:not use(west) }.
        #show.
        #show selection(S,C+2):use(S),site(S,C).
    ";
    let input = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    let config = SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Eager,
        oracle: Oracle::Countermodel,
        models: 0,
        ..SolveConfig::default()
    };
    let family = Session::builder(PreparedInput::formula(&input), config, Control::default())
        .collect(WorldViewLimits::default())?;
    assert_eq!(family.len(), 2);

    let mut observed = Vec::new();
    for answer in family.answer_sets() {
        // The complete interpretation still contains both site facts.
        assert_eq!(answer.interpretation().atoms().len(), 3);
        let display = input.metadata().observations().render(
            answer.interpretation(),
            input.metadata().output(),
            observation::Limits::default(),
            &Control::default(),
        )?;
        observed.push((
            display.text().to_owned(),
            answer
                .score()
                .expect("an admitted objective")
                .costs()
                .to_vec(),
        ));
    }
    observed.sort();
    assert_eq!(
        observed,
        [
            ("selection(east,5)".into(), vec![(1, 5)]),
            ("selection(west,3)".into(), vec![(1, 1)]),
        ]
    );
    assert_eq!(family.outcome().completion(), Some(Completion::Exhausted));
    Ok(())
}
// ANCHOR_END: example
