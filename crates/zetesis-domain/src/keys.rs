//! Keyed relations: a value that is a function of its key.
//!
//! The choice rule `1 { p(K, V) : c(V, K) } 1 :- b(K).` holds, in every answer
//! set, exactly one atom `p(k, v)` for every `k` that `b` admits and no atom
//! of `p` for any other `k`, provided the rule is `p`'s only producer. `K`
//! are the key positions, `V` the value position, and the relation is total
//! over its body. A consumer that knows the one value a constraint demands
//! can then ask for that atom instead of reading every value.
//!
//! The analysis is syntactic and conservative: a relation with any other
//! producer, a choice with other bounds, more than one element, a value
//! bound outside its condition, a body binding more than the key, or a
//! negated literal anywhere yields no key. It reads the program as given and
//! claims nothing about the program's answer sets beyond the statement above.

use std::collections::{BTreeMap, BTreeSet};

use themelios_program::program::{
    Arguments, Atom, Body, BodyElement, Choice, Condition, DefaultNegation, Guard, Head, Literal,
    LiteralInner, Program, Relation, Statement,
};
use themelios_program::provenance::WithProvenance;
use themelios_program::symbol::{Signature, Symbol, VarName};
use themelios_program::term::{Term, Variable};

use crate::limits::{Resource, Stop, check};

/// A relation whose value position is a function of its key positions,
/// total over the body of the one choice rule that produces it.
#[derive(Debug)]
pub struct KeyedRelation<'p> {
    signature: Signature,
    /// The key variable at each argument position; `None` at the value.
    arguments: Vec<Option<&'p VarName>>,
    value: usize,
    value_variable: &'p VarName,
    condition: &'p Condition,
    body: &'p Body,
}

impl<'p> KeyedRelation<'p> {
    /// The relation's signed signature.
    #[must_use]
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// The value position.
    #[must_use]
    pub fn value_position(&self) -> usize {
        self.value
    }

    /// The key variable at an argument position; `None` at the value position.
    #[must_use]
    pub fn key_variable(&self, position: usize) -> Option<&'p VarName> {
        self.arguments.get(position).copied().flatten()
    }

    /// The variable the condition binds at the value position.
    #[must_use]
    pub fn value_variable(&self) -> &'p VarName {
        self.value_variable
    }

    /// The element's condition, which binds the value.
    #[must_use]
    pub fn condition(&self) -> &'p Condition {
        self.condition
    }

    /// The rule's body, whose variables are exactly the key variables.
    #[must_use]
    pub fn body(&self) -> &'p Body {
        self.body
    }
}

/// Every keyed relation of the program, in statement order.
///
/// # Errors
/// Returns the work stop when the inspection exceeds `work`'s ceiling; no
/// partial result is published, and the steps before the stop stay spent.
pub fn keys<'p>(program: &'p Program, work: &mut KeyWork) -> Result<Vec<KeyedRelation<'p>>, Stop> {
    let mut producers: BTreeMap<Signature, usize> = BTreeMap::new();
    let mut opaque = false;
    for carrier in program.statements() {
        work.step()?;
        match carrier.get() {
            Statement::Rule(rule) => match rule.head().get() {
                Head::Literal(literal) => produced(literal, &mut producers, work)?,
                Head::Choice(choice) => {
                    for element in choice.elements() {
                        produced(element.get().literal(), &mut producers, work)?;
                    }
                }
                Head::Disjunction(disjunction) => {
                    for element in disjunction.elements() {
                        produced(element.get().literal(), &mut producers, work)?;
                    }
                }
                Head::Falsum | Head::Verum => {}
                Head::Aggregate(_) | Head::TheoryAtom(_) => opaque = true,
            },
            Statement::External(external) => {
                atom_produced(external.atom().get(), &mut producers, work)?;
            }
            _ => {}
        }
    }
    if opaque {
        return Ok(Vec::new());
    }
    let mut keys = Vec::new();
    for carrier in program.statements() {
        work.step()?;
        let Statement::Rule(rule) = carrier.get() else {
            continue;
        };
        let Head::Choice(choice) = rule.head().get() else {
            continue;
        };
        if let Some(key) = key(choice, rule.body().get(), &producers, work)? {
            keys.push(key);
        }
    }
    Ok(keys)
}

/// The steps one key analysis and its readings of facts may spend, and the
/// steps spent so far, under one ceiling: a statement read, a head atom
/// registered, a body literal or a term inspected is one step each.
#[derive(Debug)]
pub struct KeyWork {
    steps: u64,
    limit: u64,
}

impl KeyWork {
    /// Work up to `limit` steps, none spent yet.
    #[must_use]
    pub const fn new(limit: u64) -> Self {
        Self { steps: 0, limit }
    }

    /// The steps spent, by a completed or a stopped reading alike.
    #[must_use]
    pub const fn steps(&self) -> u64 {
        self.steps
    }

    fn step(&mut self) -> Result<(), Stop> {
        let observed = u128::from(self.steps) + 1;
        check(Resource::Work, observed, u128::from(self.limit))?;
        self.steps += 1;
        Ok(())
    }
}

/// The values at `position` of the facts of `signature`, when facts are its
/// only producers: `None` when a rule with a body, a choice, a disjunction,
/// an aggregate or theory head, or an external declaration produces it, or
/// a fact holds something other than a scalar at that position. What the
/// facts admit is then exactly what the relation admits; a consumer that
/// needs a bound on a keyed relation's condition reads it here rather than
/// from an analysis of derived values. One step per statement read.
///
/// # Errors
/// Returns the work stop; the steps before it stay spent.
pub fn facts<'p>(
    program: &'p Program,
    signature: &Signature,
    position: usize,
    work: &mut KeyWork,
) -> Result<Option<Vec<&'p Symbol>>, Stop> {
    let mut values = Vec::new();
    for carrier in program.statements() {
        work.step()?;
        match carrier.get() {
            Statement::Rule(rule) => match rule.head().get() {
                Head::Literal(literal) => {
                    let LiteralInner::Atom(atom) = &literal.inner else {
                        continue;
                    };
                    let atom = atom.get();
                    for terms in atom.alternatives() {
                        if atom_signature(atom, terms.len()).as_ref() != Some(signature) {
                            continue;
                        }
                        if literal.negation != DefaultNegation::None
                            || rule.body().get().elements().next().is_some()
                        {
                            return Ok(None);
                        }
                        match terms.get(position) {
                            Some(Term::Symbolic(symbol)) => values.push(symbol),
                            _ => return Ok(None),
                        }
                    }
                }
                Head::Choice(choice)
                    if choice
                        .elements()
                        .any(|element| names(element.get().literal(), signature)) =>
                {
                    return Ok(None);
                }
                Head::Disjunction(disjunction)
                    if disjunction
                        .elements()
                        .any(|element| names(element.get().literal(), signature)) =>
                {
                    return Ok(None);
                }
                Head::Choice(_) | Head::Disjunction(_) | Head::Falsum | Head::Verum => {}
                Head::Aggregate(_) | Head::TheoryAtom(_) => return Ok(None),
            },
            Statement::External(external) if atom_names(external.atom().get(), signature) => {
                return Ok(None);
            }
            _ => {}
        }
    }
    Ok(Some(values))
}

/// Whether the literal's atom has `signature` under any of its alternatives.
fn names(literal: &Literal, signature: &Signature) -> bool {
    match &literal.inner {
        LiteralInner::Atom(atom) => atom_names(atom.get(), signature),
        _ => false,
    }
}

fn atom_names(atom: &Atom, signature: &Signature) -> bool {
    atom.alternatives()
        .any(|terms| atom_signature(atom, terms.len()).as_ref() == Some(signature))
}

fn produced(
    literal: &Literal,
    producers: &mut BTreeMap<Signature, usize>,
    work: &mut KeyWork,
) -> Result<(), Stop> {
    if let LiteralInner::Atom(atom) = &literal.inner {
        atom_produced(atom.get(), producers, work)?;
    }
    Ok(())
}

fn atom_produced(
    atom: &Atom,
    producers: &mut BTreeMap<Signature, usize>,
    work: &mut KeyWork,
) -> Result<(), Stop> {
    for terms in atom.alternatives() {
        work.step()?;
        let Some(signature) = atom_signature(atom, terms.len()) else {
            continue;
        };
        *producers.entry(signature).or_default() += 1;
    }
    Ok(())
}

/// The signed signature of `atom` with `arity` arguments, or `None` when the
/// arity does not fit a signature's width: no signature names such an atom,
/// so it produces nothing and is keyed by nothing.
#[must_use]
pub fn atom_signature(atom: &Atom, arity: usize) -> Option<Signature> {
    Some(Signature {
        sign: atom.sign,
        name: atom.name.clone(),
        arity: u32::try_from(arity).ok()?,
    })
}

fn key<'p>(
    choice: &'p Choice,
    body: &'p Body,
    producers: &BTreeMap<Signature, usize>,
    work: &mut KeyWork,
) -> Result<Option<KeyedRelation<'p>>, Stop> {
    if !exactly_one(choice.left_guard(), choice.right_guard()) {
        return Ok(None);
    }
    let mut elements = choice.elements();
    let (Some(element), None) = (elements.next(), elements.next()) else {
        return Ok(None);
    };
    let element = element.get();
    let literal = element.literal();
    let LiteralInner::Atom(atom) = &literal.inner else {
        return Ok(None);
    };
    let atom = atom.get();
    let Arguments::Single(terms) = &atom.arguments else {
        return Ok(None);
    };
    if literal.negation != DefaultNegation::None {
        return Ok(None);
    }
    let Some(keyed) = atom_signature(atom, terms.len()) else {
        return Ok(None);
    };
    if producers.get(&keyed) != Some(&1) {
        return Ok(None);
    }
    let Some(key_variables) = key_variables(body, &keyed, work)? else {
        return Ok(None);
    };
    let Some(positions) = value_position(terms, &key_variables, work)? else {
        return Ok(None);
    };
    if !binds_value(
        element.condition(),
        &keyed,
        &key_variables,
        positions.variable,
        work,
    )? {
        return Ok(None);
    }
    Ok(Some(KeyedRelation {
        signature: keyed,
        arguments: positions.arguments,
        value: positions.value,
        value_variable: positions.variable,
        condition: element.condition(),
        body,
    }))
}

/// The body's variables when it binds exactly a key: positive flat atoms over
/// variables and ground terms, none naming the keyed relation.
fn key_variables<'p>(
    body: &'p Body,
    keyed: &Signature,
    work: &mut KeyWork,
) -> Result<Option<BTreeSet<&'p VarName>>, Stop> {
    let mut variables = BTreeSet::new();
    for element in body.elements() {
        work.step()?;
        let Some(atom) = positive_atom(element.get()) else {
            return Ok(None);
        };
        let Arguments::Single(terms) = &atom.arguments else {
            return Ok(None);
        };
        if atom_signature(atom, terms.len()).as_ref() == Some(keyed) {
            return Ok(None);
        }
        for term in terms {
            work.step()?;
            match term {
                Term::Variable(Variable::Named(name)) => {
                    variables.insert(name);
                }
                Term::Symbolic(_) => {}
                _ => return Ok(None),
            }
        }
    }
    Ok(Some(variables))
}

/// The head's arguments read as a key around one value.
struct Positions<'p> {
    arguments: Vec<Option<&'p VarName>>,
    value: usize,
    variable: &'p VarName,
}

/// The head arguments as key variables around one value variable, when every
/// argument is a distinct variable and every key variable occurs.
fn value_position<'p>(
    terms: &'p [Term],
    key_variables: &BTreeSet<&'p VarName>,
    work: &mut KeyWork,
) -> Result<Option<Positions<'p>>, Stop> {
    let mut arguments = Vec::with_capacity(terms.len());
    let mut seen = BTreeSet::new();
    let mut value = None;
    for (position, term) in terms.iter().enumerate() {
        work.step()?;
        let Term::Variable(Variable::Named(name)) = term else {
            return Ok(None);
        };
        if !seen.insert(name) {
            return Ok(None);
        }
        if key_variables.contains(name) {
            arguments.push(Some(name));
        } else if value.is_none() {
            value = Some((position, name));
            arguments.push(None);
        } else {
            return Ok(None);
        }
    }
    let Some((value, variable)) = value else {
        return Ok(None);
    };
    if seen.len() != key_variables.len() + 1 {
        return Ok(None);
    }
    Ok(Some(Positions {
        arguments,
        value,
        variable,
    }))
}

/// Whether the condition binds the value with positive flat atoms that read
/// only the value and the key and name another relation than the keyed one.
fn binds_value(
    condition: &Condition,
    keyed: &Signature,
    key_variables: &BTreeSet<&VarName>,
    value_variable: &VarName,
    work: &mut KeyWork,
) -> Result<bool, Stop> {
    let mut bound = false;
    for literal in condition.literals() {
        work.step()?;
        let Some(atom) = positive_atom_literal(literal.get()) else {
            return Ok(false);
        };
        let Arguments::Single(terms) = &atom.arguments else {
            return Ok(false);
        };
        if atom_signature(atom, terms.len()).as_ref() == Some(keyed) {
            return Ok(false);
        }
        for term in terms {
            work.step()?;
            match term {
                Term::Variable(Variable::Named(name)) if name == value_variable => bound = true,
                Term::Variable(Variable::Named(name)) if key_variables.contains(name) => {}
                Term::Symbolic(_) => {}
                _ => return Ok(false),
            }
        }
    }
    Ok(bound)
}

/// `1 { … } 1`, `1 <= { … } <= 1`, `{ … } = 1` or `1 = { … }`.
fn exactly_one(
    left: Option<&WithProvenance<Guard>>,
    right: Option<&WithProvenance<Guard>>,
) -> bool {
    let one = |guard: &WithProvenance<Guard>, relations: &[Option<Relation>]| {
        relations.contains(&guard.get().relation)
            && matches!(guard.get().term, Term::Symbolic(Symbol::Number(1)))
    };
    match (left, right) {
        (Some(left), Some(right)) => {
            one(left, &[None, Some(Relation::Le)]) && one(right, &[None, Some(Relation::Le)])
        }
        (Some(guard), None) | (None, Some(guard)) => one(guard, &[Some(Relation::Eq)]),
        (None, None) => false,
    }
}

fn positive_atom(element: &BodyElement) -> Option<&Atom> {
    let BodyElement::Literal(literal) = element else {
        return None;
    };
    positive_atom_literal(literal)
}

fn positive_atom_literal(literal: &Literal) -> Option<&Atom> {
    if literal.negation != DefaultNegation::None {
        return None;
    }
    let LiteralInner::Atom(atom) = &literal.inner else {
        return None;
    };
    Some(atom.get())
}
