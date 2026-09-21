//! Reuse query preparation and empty workspace capacity across frozen seeds.

// ANCHOR: example
use zetesis_core::{
    AdmissionLimits, AtomPattern, ConstructionError, Model, Predicate, Program, Seed, Template,
    Term, Value,
};
use zetesis_cpu::{
    Cancellation, ClosureWorkspace, Limits, PreparationLimits, PreparedQueries, check,
};

fn pattern(name: &str, term: Term) -> Result<AtomPattern, ConstructionError> {
    AtomPattern::new(Predicate::new(name, 1)?, vec![term])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domain = pattern("d", Term::Variable(0))?;
    let left = pattern("left", Term::Variable(0))?;
    let right = pattern("right", Term::Variable(0))?;
    // d(1). left(X) :- d(X), not right(X). right(X) :- d(X), not left(X).
    let program = Program::new(
        vec![
            Template::new(
                Some(pattern("d", Term::Constant(Value::Number(1)))?),
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            Template::new(
                Some(left.clone()),
                vec![domain.clone()],
                vec![],
                vec![right.clone()],
                vec![],
            ),
            Template::new(
                Some(right.clone()),
                vec![domain.clone()],
                vec![],
                vec![left.clone()],
                vec![],
            ),
        ],
        AdmissionLimits::default(),
    )?;
    let assignment = [Value::Number(1)];
    let left_atom = left.instantiate(&assignment)?;
    let right_atom = right.instantiate(&assignment)?;
    let first_seed = Seed::new(&program, [left_atom.clone()])?;
    let second_seed = Seed::new(&program, [right_atom.clone()])?;
    let cancellation = Cancellation::default();
    let prepared = PreparedQueries::new(&program, PreparationLimits::default(), &cancellation)?;
    let mut workspace = ClosureWorkspace::default();
    let limits = Limits::default();

    let first = prepared.check_view(first_seed.view(), &mut workspace, limits, &cancellation)?;
    let second = prepared.check_view(second_seed.view(), &mut workspace, limits, &cancellation)?;
    // Both results remain independent after the workspace serves another seed.
    for (result, seed, expected) in [
        (
            &first,
            &first_seed,
            Model::new([domain.instantiate(&assignment)?, left_atom]),
        ),
        (
            &second,
            &second_seed,
            Model::new([domain.instantiate(&assignment)?, right_atom]),
        ),
    ] {
        let fresh = check(&program, seed, limits, &cancellation)?;
        assert!(result.program().same_instance(&program));
        assert!(result.accepted());
        assert_eq!(result.closure(), &expected);
        assert_eq!(result.closure(), fresh.closure());
        assert_eq!(result.constraint_violated(), fresh.constraint_violated());
        assert_eq!(result.seed_mismatch(), fresh.seed_mismatch());
    }
    Ok(())
}
// ANCHOR_END: example
