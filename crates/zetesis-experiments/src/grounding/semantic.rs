//! Complete native result comparison outside every admission interval.

use std::fmt::Write;

use serde::Serialize;
use zetesis_sat::{Control, StableModels, Statistics};
use zetesis_themelios::AdmittedFormulaBundle;

use super::{Configuration, Error, storage};

/// A sorted full-model multiset, or its explicitly incomplete retained prefix.
/// Indices refer to the report's complete ordered atom catalog, including hidden
/// atoms and classical signs. Sorting never removes duplicate interpretations.
#[derive(Debug, Default, Serialize)]
pub struct Models {
    /// Each vector contains every true atom index in ascending order.
    pub interpretations: Vec<Vec<usize>>,
    /// True only after native search proves exhaustion under unchanged restrictions.
    pub exhausted: bool,
    /// Native verified model count, including a model refused by capture storage.
    pub verified_models: u64,
    /// Number of atom indices retained in the interpretation vectors.
    pub retained_atom_indices: usize,
    /// Native cumulative search and optional certificate work.
    pub search_work: u64,
    /// Native decisions across complete enumeration or its failed prefix.
    pub search_decisions: u64,
    /// Native classical candidates checked for membership.
    pub candidates: u64,
    /// Native proper-subset queries performed.
    pub countermodel_queries: u64,
}

impl Models {
    fn update(&mut self, statistics: &Statistics, exhausted: bool) {
        self.exhausted = exhausted;
        self.verified_models = statistics.stable_models;
        self.search_work = statistics.search.work;
        self.search_decisions = statistics.search.decisions;
        self.candidates = statistics.candidates;
        self.countermodel_queries = statistics.countermodel_queries;
        self.interpretations.sort_unstable();
    }
}

pub(super) fn enumerate(
    subject: &AdmittedFormulaBundle,
    config: &Configuration,
) -> (Models, Option<Error>) {
    let mut retained = Models::default();
    let outcome = enumerate_into(subject, config, &mut retained);
    (retained, outcome.err())
}

fn enumerate_into(
    subject: &AdmittedFormulaBundle,
    config: &Configuration,
    retained: &mut Models,
) -> Result<(), Error> {
    retained.interpretations = storage::reserve(config.capture.max_models)?;
    let mut search = StableModels::new(subject.theory(), config.search, Control::default())
        .map_err(Error::Search)?;
    let outcome = search
        .enable_certified_checking(config.certificate)
        .map_err(Error::Search)
        .and_then(|_| retain_models(&mut search, config, retained));
    retained.update(&search.statistics(), search.exhausted());
    outcome?;
    if !retained.exhausted {
        return Err(Error::NotExhausted);
    }
    Ok(())
}

fn retain_models(
    search: &mut StableModels,
    config: &Configuration,
    retained: &mut Models,
) -> Result<(), Error> {
    // Each successful iteration consumes one native verified result, bounded by
    // the search candidate/work limits and the retained model ceiling. None is
    // not accepted as completion until the caller inspects exhausted().
    while let Some(next) = search.next_verified() {
        let model = next.map_err(Error::Search)?;
        if retained.interpretations.len() == config.capture.max_models {
            return Err(Error::Limit {
                resource: "models",
                limit: config.capture.max_models,
            });
        }
        let count = model.interpretation().atoms().count();
        let total = retained
            .retained_atom_indices
            .checked_add(count)
            .filter(|&total| total <= config.capture.max_model_atoms)
            .ok_or(Error::Limit {
                resource: "model_atom_indices",
                limit: config.capture.max_model_atoms,
            })?;
        let mut atoms = storage::reserve(count)?;
        atoms.extend(model.interpretation().atoms());
        retained.interpretations.push(atoms);
        retained.retained_atom_indices = total;
    }
    Ok(())
}

pub(super) fn catalog(subject: &AdmittedFormulaBundle, limit: usize) -> Result<Vec<String>, Error> {
    let mut atoms = storage::reserve(subject.atoms().len())?;
    let mut used = 0;
    for atom in subject.atoms() {
        let mut bytes = storage::Bytes::new(limit - used, "atom_text_bytes");
        let outcome = write_atom(&mut bytes, atom);
        if let Some(error) = bytes.refusal {
            return Err(match error {
                Error::Limit { resource, .. } => Error::Limit { resource, limit },
                other => other,
            });
        }
        outcome.map_err(|_| Error::Formatting)?;
        used += bytes.data.len();
        // fmt::Write accepts only UTF-8 fragments; this conversion moves storage.
        atoms.push(String::from_utf8(bytes.data).map_err(|_| Error::Formatting)?);
    }
    Ok(atoms)
}

fn write_atom(output: &mut impl Write, atom: &zetesis_core::Atom) -> std::fmt::Result {
    use zetesis_core::{Sign, Value};
    if atom.predicate().sign() == Sign::Negative {
        output.write_char('-')?;
    }
    output.write_str(atom.predicate().name())?;
    if atom.values().is_empty() {
        return Ok(());
    }
    output.write_char('(')?;
    for (index, value) in atom.values().iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        match value {
            Value::Infimum => output.write_str("#inf")?,
            Value::Supremum => output.write_str("#sup")?,
            Value::Number(number) => write!(output, "{number}")?,
            Value::Symbol(symbol) => output.write_str(symbol)?,
            Value::Structured(value) => write!(output, "{value}")?,
            Value::String(text) => {
                output.write_char('"')?;
                // Source-admitted strings use precisely these three ASP escapes.
                // Literal tabs stay literal, distinct from Rust Debug spelling.
                for character in text.chars() {
                    match character {
                        '"' => output.write_str("\\\"")?,
                        '\\' => output.write_str("\\\\")?,
                        '\n' => output.write_str("\\n")?,
                        other => output.write_char(other)?,
                    }
                }
                output.write_char('"')?;
            }
        }
    }
    output.write_char(')')
}

pub(super) fn same_subject(a: &AdmittedFormulaBundle, b: &AdmittedFormulaBundle) -> bool {
    a.atoms() == b.atoms()
        && a.theory().nodes() == b.theory().nodes()
        && a.theory().roots() == b.theory().roots()
        && a.formula_origins() == b.formula_origins()
        && a.objective_origins() == b.objective_origins()
        && a.objective_declarations() == b.objective_declarations()
        && a.objectives().templates() == b.objectives().templates()
        && a.metadata() == b.metadata()
}

pub(super) fn objective_free(subject: &AdmittedFormulaBundle) -> Result<(), Error> {
    if !subject.objective_declarations().is_empty() || subject.objectives().is_present() {
        return Err(Error::Objective);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use zetesis_core::{Atom, Predicate, Sign, Value, ValueLimits, ValueNode};

    #[test]
    fn catalog_spelling_preserves_closed_value_identity() {
        // These raw core values exercise formatting independently of which
        // source profiles currently admit extrema or constructed expressions.
        let structured = Value::from_nodes(
            vec![
                ValueNode::Function {
                    name: "f".into(),
                    sign: Sign::Negative,
                    arity: 2,
                },
                ValueNode::Number(1),
                ValueNode::Tuple { arity: 2 },
                ValueNode::Number(2),
                ValueNode::Number(3),
            ],
            ValueLimits::default(),
        )
        .unwrap();
        let values = vec![
            Value::Infimum,
            Value::String("#inf".into()),
            Value::Supremum,
            Value::String("#sup".into()),
            structured,
            Value::Symbol("a".into()),
        ];
        let atom = Atom::new(
            Predicate::with_sign("p", values.len(), Sign::Negative).unwrap(),
            values,
        )
        .unwrap();
        let mut text = String::new();
        super::write_atom(&mut text, &atom).unwrap();
        assert_eq!(text, "-p(#inf,\"#inf\",#sup,\"#sup\",-f(1,(2,3)),a)");
    }
}
