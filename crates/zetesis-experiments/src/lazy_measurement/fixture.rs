use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Predicate, Program, Seed, Template, Term, Value,
};

use super::{Case, Error, Family};

pub(super) struct Fixture {
    pub program: Program,
    pub seeds: Vec<Seed>,
}

fn pattern(name: &str, terms: Vec<Term>) -> Result<AtomPattern, Error> {
    let predicate = Predicate::new(name, terms.len()).map_err(Error::Construction)?;
    AtomPattern::new(predicate, terms).map_err(Error::Construction)
}

fn constant(name: &str, value: i32) -> Result<AtomPattern, Error> {
    pattern(name, vec![Term::Constant(Value::Number(value))])
}

pub(super) fn build(case: Case) -> Result<Fixture, Error> {
    let width = i32::try_from(case.width.get())
        .map_err(|_| Error::Configuration("width is not an integer"))?;
    let mut templates = Vec::new();
    for value in 0..width {
        for name in ["pick", "a", "b", "c"] {
            templates.push(Template::new(
                Some(constant(name, value)?),
                vec![],
                vec![constant("pick", value)?],
                vec![],
                vec![],
            ));
        }
    }
    templates.push(Template::new(
        Some(pattern("triple", (0..3).map(Term::Variable).collect())?),
        ["a", "b", "c"]
            .into_iter()
            .enumerate()
            .map(|(variable, name)| pattern(name, vec![Term::Variable(variable)]))
            .collect::<Result<_, _>>()?,
        vec![],
        vec![],
        vec![],
    ));
    // Include a rejection control without changing the positive join product.
    templates.push(Template::new(
        None,
        vec![pattern(
            "triple",
            vec![Term::Constant(Value::Number(0)); 3],
        )?],
        vec![],
        vec![],
        vec![],
    ));
    let program = Program::new(templates, AdmissionLimits::default()).map_err(Error::Admission)?;
    let mut seeds = Vec::with_capacity(case.worlds.get());
    for world in 0..case.worlds.get() {
        let selected = i32::try_from(world % case.width.get())
            .map_err(|_| Error::Configuration("world index is not an integer"))?;
        let atoms = (0..width)
            .filter(|value| case.family == Family::Dense || *value == selected)
            .map(|value| Atom::new(Predicate::new("pick", 1)?, vec![Value::Number(value)]))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Error::Construction)?;
        seeds.push(Seed::new(&program, atoms).map_err(Error::Seed)?);
    }
    Ok(Fixture { program, seeds })
}
