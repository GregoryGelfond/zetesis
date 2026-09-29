//! Builders of small test programs over numbers.

use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Template, Term, Value};

/// The atom pattern `name(terms…)`, whose arity is the number of terms.
///
/// # Panics
/// Panics if `name` is empty.
#[must_use]
pub fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    let predicate = Predicate::new(name, terms.len()).expect("nonempty test name");
    AtomPattern::new(predicate, terms).expect("matching arity")
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
    fn a_program_holds_the_facts_it_is_built_from() {
        let built = program(vec![
            fact("p", vec![number(1)]),
            fact("q", vec![number(2), number(3)]),
        ]);
        assert_eq!(built.templates().len(), 2);
    }
}
