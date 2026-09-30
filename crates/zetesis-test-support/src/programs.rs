//! Builders of small test atoms and programs.

use zetesis_core::{AdmissionLimits, Atom, AtomPattern, Predicate, Program, Template, Term, Value};

/// The atom `name(values…)`, whose arity is the number of values.
///
/// # Panics
/// Panics if `name` is empty.
#[must_use]
pub fn atom(name: &str, values: Vec<Value>) -> Atom {
    let predicate = Predicate::new(name, values.len()).expect("nonempty test name");
    Atom::new(predicate, values).expect("matching arity")
}

/// The atom pattern `name(terms…)`, whose arity is the number of terms.
///
/// # Panics
/// Panics if `name` is empty.
#[must_use]
pub fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    let predicate = Predicate::new(name, terms.len()).expect("nonempty test name");
    AtomPattern::new(predicate, terms).expect("matching arity")
}

/// The nullary atom `name`.
///
/// # Panics
/// Panics if `name` is empty.
#[must_use]
pub fn nullary(name: &str) -> Atom {
    atom(name, Vec::new())
}

/// The nullary atom pattern `name`.
///
/// # Panics
/// Panics if `name` is empty.
#[must_use]
pub fn nullary_pattern(name: &str) -> AtomPattern {
    pattern(name, Vec::new())
}

/// The number `value` as a constant term.
#[must_use]
pub fn number(value: i32) -> Term {
    Term::Constant(Value::Number(value))
}

/// The fact `name(values…).`
///
/// # Panics
/// Panics if `name` is empty.
#[must_use]
pub fn fact(name: &str, values: Vec<Term>) -> Template {
    Template::new(Some(pattern(name, values)), vec![], vec![], vec![], vec![])
}

/// The program of `templates` under the default admission limits.
///
/// # Panics
/// Panics if the templates are not a safe, finite program.
#[must_use]
pub fn program(templates: Vec<Template>) -> Program {
    Program::new(templates, AdmissionLimits::default()).expect("safe finite test program")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_atom_takes_its_arity_from_its_values() {
        let built = atom("p", vec![Value::Number(1), Value::Symbol("a".into())]);
        assert_eq!(built.predicate().arity(), 2);
        assert_eq!(
            built.values(),
            [Value::Number(1), Value::Symbol("a".into())]
        );
    }

    #[test]
    fn a_nullary_atom_and_pattern_have_no_arguments() {
        assert_eq!(nullary("a").predicate().arity(), 0);
        assert!(nullary("a").values().is_empty());
        assert_eq!(nullary_pattern("a").predicate().arity(), 0);
    }

    #[test]
    fn a_program_holds_the_facts_it_is_built_from() {
        let built = program(vec![
            fact("p", vec![number(1)]),
            fact("q", vec![number(2), number(3)]),
        ]);
        assert_eq!(built.templates().len(), 2);
    }
}
