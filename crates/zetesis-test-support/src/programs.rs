//! Builders of small test atoms and programs.

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Sign, Template, Term, Value,
    ValueLimits, ValueNode,
};

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

/// The atom `name(values…)` over numbers, whose arity is the number of values.
///
/// # Panics
/// Panics if `name` is empty.
#[must_use]
pub fn numbered(name: &str, values: &[i32]) -> Atom {
    atom(name, values.iter().copied().map(Value::Number).collect())
}

/// The atom `name(value)` over one number.
///
/// # Panics
/// Panics if `name` is empty.
#[must_use]
pub fn unary(name: &str, value: i32) -> Atom {
    numbered(name, &[value])
}

/// The atom `name(values…)` whose predicate carries `sign`.
///
/// # Panics
/// Panics if `name` is empty.
#[must_use]
pub fn signed(name: &str, sign: Sign, values: Vec<Value>) -> Atom {
    let predicate = Predicate::with_sign(name, values.len(), sign).expect("nonempty test name");
    Atom::new(predicate, values).expect("matching arity")
}

/// The atom `name(values…)` over numbers whose predicate carries `sign`.
///
/// # Panics
/// Panics if `name` is empty.
#[must_use]
pub fn signed_numbered(name: &str, sign: Sign, values: &[i32]) -> Atom {
    signed(
        name,
        sign,
        values.iter().copied().map(Value::Number).collect(),
    )
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

/// The model holding the nullary atoms `names`.
///
/// # Panics
/// Panics if a name is empty.
#[must_use]
pub fn model(names: &[&str]) -> Model {
    Model::new(names.iter().map(|name| nullary(name))).expect("distinct test atoms")
}

/// The variable numbered `index` as a term.
#[must_use]
pub fn variable(index: usize) -> Term {
    Term::Variable(index)
}

/// The value of `nodes` under the default value limits.
///
/// # Panics
/// Panics if `nodes` do not form one value within those limits.
#[must_use]
pub fn value(nodes: Vec<ValueNode>) -> Value {
    Value::from_nodes(nodes, ValueLimits::default()).expect("a valid test value")
}

/// The positive function node `name` of `arity` arguments.
#[must_use]
pub fn function(name: &str, arity: usize) -> ValueNode {
    ValueNode::Function {
        name: name.into(),
        sign: Sign::Positive,
        arity,
    }
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
    fn numbered_atoms_carry_their_numbers() {
        let built = numbered("p", &[1, 2]);
        assert_eq!(built.predicate().arity(), 2);
        assert_eq!(built.values(), [Value::Number(1), Value::Number(2)]);
        assert_eq!(unary("p", 3), numbered("p", &[3]));
    }

    #[test]
    fn signed_atoms_carry_their_sign() {
        let negative = signed("q", Sign::Negative, vec![Value::Number(1)]);
        assert_eq!(negative.predicate().sign(), Sign::Negative);
        assert_eq!(negative.values(), [Value::Number(1)]);
        assert_eq!(signed_numbered("q", Sign::Negative, &[1]), negative);
        assert_eq!(unary("q", 1).predicate().sign(), Sign::Positive);
    }

    #[test]
    fn a_model_holds_its_nullary_atoms() {
        let built = model(&["a", "b"]);
        assert_eq!(built.atoms().len(), 2);
        assert!(built.contains(&nullary("a")));
        assert!(built.contains(&nullary("b")));
    }

    #[test]
    fn a_variable_term_carries_its_index() {
        assert_eq!(variable(2), Term::Variable(2));
    }

    #[test]
    fn a_function_node_is_positive_with_its_arity() {
        assert_eq!(
            function("f", 2),
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Positive,
                arity: 2,
            }
        );
    }

    #[test]
    fn a_value_is_built_from_its_nodes() {
        assert_eq!(value(vec![ValueNode::Number(3)]), Value::Number(3));
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
