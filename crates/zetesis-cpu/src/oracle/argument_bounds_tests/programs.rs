//! Builders of small test programs over numbers.

use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Template, Term, Value};

pub fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    let predicate = Predicate::new(name, terms.len()).expect("nonempty test name");
    AtomPattern::new(predicate, terms).expect("matching arity")
}

pub fn number(value: i32) -> Term {
    Term::Constant(Value::Number(value))
}

pub fn fact(name: &str, values: Vec<Term>) -> Template {
    Template::new(Some(pattern(name, values)), vec![], vec![], vec![], vec![])
}

pub fn program(templates: Vec<Template>) -> Program {
    Program::new(templates, AdmissionLimits::default()).expect("safe finite test program")
}
