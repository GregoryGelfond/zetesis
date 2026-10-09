//! Specialize fixed positive witnesses without introducing semantic atoms.
//!
//! One flat anchor and unary fact guards bind local variables not read by the
//! residual constraint. Distinct retained tuples produce residual constraints;
//! all original facts remain. On every answer set of the remaining program,
//! the original constraint fires iff one of this complete family fires.
//!
//! The private policy bounds optional preparation, not source semantics.
//! A stopped or too-large family stays written. Accepted analysis and storage
//! charges remain spent; cancellation and allocation keep their typed failures.

use themelios_program::program::{
    Arguments, Atom, Body, BodyElement, DefaultNegation, Head, Literal, LiteralInner, Program,
    Rule, Statement,
};
use themelios_program::symbol::{Symbol, VarName};
use themelios_program::term::{Term, Variable};
use themelios_program::unify::{mgu, substitute};
use zetesis_domain::{FactIndex, KeyWork, atom_signature};

use crate::expansion::Budget;
use crate::formula_keys::owners::Owners;
use crate::formula_rewrite::{FixedFacts, Replacement, Replacements};
use crate::{ExpansionFailure, ExpansionResource, FormulaFailure, ProgramSite};

// Optional preparation cost policy. Neither cap establishes an
// empty relation. The existing cumulative budgets may be tighter. In particular,
// more templates can cost more than the eliminated witnesses save.
#[derive(Clone, Copy)]
struct Policy {
    work: u64,
    rules: usize,
}
const POLICY: Policy = Policy {
    work: 100_000,
    rules: 256,
};
const TAG: &str = "zetesis-fixed-constraint";

enum Failure {
    Optional,
    Source(FormulaFailure),
}

impl From<FormulaFailure> for Failure {
    fn from(error: FormulaFailure) -> Self {
        Self::Source(error)
    }
}
impl From<ExpansionFailure> for Failure {
    fn from(error: ExpansionFailure) -> Self {
        Self::Source(error.into())
    }
}

struct Reading<'a> {
    work: &'a mut KeyWork,
    budget: &'a mut Budget,
    site: ProgramSite,
}

impl Reading<'_> {
    fn step(&mut self) -> Result<(), Failure> {
        self.budget.poll(self.site)?;
        self.work.step().map_err(|_| Failure::Optional)
    }

    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Failure> {
        self.step()?;
        crate::formula_pattern::push(values, value, self.budget, self.site)?;
        Ok(())
    }

    fn family(&mut self, bytes: u128) -> Result<bool, Failure> {
        match self.budget.check_family(bytes, self.site) {
            Ok(()) => Ok(true),
            Err(ExpansionFailure::Limit {
                resource: ExpansionResource::FamilyBytes,
                ..
            }) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

pub(crate) fn specialize(
    program: &Program,
    owners: &Owners,
    facts: &mut FixedFacts<'_>,
    budget: &mut Budget,
    site: ProgramSite,
) -> Result<Replacements, FormulaFailure> {
    with_policy(program, owners, facts, budget, site, POLICY)
}

fn with_policy(
    program: &Program,
    owners: &Owners,
    facts: &mut FixedFacts<'_>,
    budget: &mut Budget,
    site: ProgramSite,
    policy: Policy,
) -> Result<Replacements, FormulaFailure> {
    let mut work = KeyWork::new(policy.work.min(budget.remaining_term_work()));
    let mut replacements = Replacements::new();
    let mut remaining = policy.rules.min(budget.remaining_templates());
    let result = (|| {
        let mut reading = Reading {
            work: &mut work,
            budget: &mut *budget,
            site,
        };
        for (position, carrier) in program.statements().enumerate() {
            reading.site = site;
            reading.step()?;
            if remaining == 0 {
                break;
            }
            let Some(owner) = owners.at(position) else {
                continue;
            };
            let Statement::Rule(rule) = carrier.get() else {
                continue;
            };
            if !matches!(rule.head().get(), Head::Falsum) {
                continue;
            }
            reading.site = crate::extended::origin(carrier, site.with_statement(owner));
            let Some(literals) = positive_body(rule, &mut reading)? else {
                continue;
            };
            if let Some(rules) = family(&literals, facts, remaining, &mut reading)? {
                remaining -= rules.len();
                replacements.insert(
                    owner,
                    Replacement {
                        provenance: carrier.provenance().clone(),
                        rules,
                        tag: TAG,
                    },
                );
            }
        }
        Ok::<(), Failure>(())
    })();
    budget.charge(ExpansionResource::TermWork, u128::from(work.steps()), site)?;
    match result {
        Ok(()) | Err(Failure::Optional) => Ok(replacements),
        Err(Failure::Source(error)) => Err(error),
    }
}

fn positive_body<'a>(
    rule: &'a Rule,
    reading: &mut Reading<'_>,
) -> Result<Option<Vec<&'a Literal>>, Failure> {
    let mut literals = Vec::new();
    for element in rule.body().get().elements() {
        reading.step()?;
        let BodyElement::Literal(literal) = element.get() else {
            return Ok(None);
        };
        if literal.negation != DefaultNegation::None {
            return Ok(None);
        }
        let LiteralInner::Atom(atom) = &literal.inner else {
            return Ok(None);
        };
        let Arguments::Single(terms) = &atom.get().arguments else {
            return Ok(None);
        };
        for term in terms.iter().flat_map(Term::subterms) {
            reading.step()?;
            if !matches!(
                term,
                Term::Variable(Variable::Named(_))
                    | Term::Symbolic(_)
                    | Term::Function { .. }
                    | Term::Tuple(_)
            ) {
                return Ok(None);
            }
        }
        reading.push(&mut literals, literal)?;
    }
    Ok((literals.len() > 1).then_some(literals))
}

fn atom(literal: &Literal) -> &Atom {
    let LiteralInner::Atom(atom) = &literal.inner else {
        unreachable!("checked positive atom")
    };
    atom.get()
}

fn terms(atom: &Atom) -> &[Term] {
    let Arguments::Single(terms) = &atom.arguments else {
        unreachable!("checked single argument tuple")
    };
    terms
}

fn atomic(value: &Symbol) -> bool {
    match value {
        Symbol::Function { arguments, .. } => arguments.is_empty(),
        Symbol::Tuple(_) => false,
        _ => true,
    }
}

fn fixed<'a, 'p>(
    atom: &Atom,
    facts: &'a FactIndex<'p>,
    reading: &mut Reading<'_>,
) -> Result<Option<&'a [&'p [Term]]>, Failure> {
    let Some(signature) = atom_signature(atom, terms(atom).len()) else {
        return Ok(None);
    };
    reading.budget.poll(reading.site)?;
    facts
        .tuples(&signature, reading.work)
        .map_err(|_| Failure::Optional)
}

struct Block<'a, 'p> {
    anchor: usize,
    retained: Vec<&'a VarName>,
    guards: Vec<(usize, &'a VarName, &'a [&'p [Term]])>,
    rows: &'a [&'p [Term]],
}

/// Necessary anchor shape, independent of whether a predicate is fixed.
/// This is the same charged reading used by complete block recognition.
fn anchor_variables<'a>(
    anchor: &'a Atom,
    reading: &mut Reading<'_>,
) -> Result<Option<Vec<&'a VarName>>, Failure> {
    let mut variables = Vec::new();
    for term in terms(anchor) {
        reading.step()?;
        match term {
            Term::Variable(Variable::Named(name)) => {
                if !variables.contains(&name) {
                    reading.push(&mut variables, name)?;
                }
            }
            Term::Symbolic(value) if atomic(value) => {}
            _ => return Ok(None),
        }
    }
    if variables.len() < 2 {
        return Ok(None);
    }
    Ok(Some(variables))
}

fn block<'a, 'p>(
    literals: &'a [&'a Literal],
    anchor: usize,
    variables: &[&'a VarName],
    facts: &'a FactIndex<'p>,
    reading: &mut Reading<'_>,
) -> Result<Option<Block<'a, 'p>>, Failure> {
    let anchor_atom = atom(literals[anchor]);
    let Some(rows) = fixed(anchor_atom, facts, reading)? else {
        return Ok(None);
    };
    let mut retained = Vec::new();
    let mut guards = Vec::new();
    for (position, literal) in literals.iter().enumerate() {
        reading.step()?;
        if position == anchor {
            continue;
        }
        let argument = atom(literal);
        if let [Term::Variable(Variable::Named(name))] = terms(argument)
            && variables.contains(&name)
            && let Some(rows) = fixed(argument, facts, reading)?
        {
            reading.push(&mut guards, (position, name, rows))?;
            continue;
        }
        for term in terms(argument).iter().flat_map(Term::subterms) {
            reading.step()?;
            if let Term::Variable(Variable::Named(name)) = term
                && variables.contains(&name)
                && !retained.contains(&name)
            {
                reading.push(&mut retained, name)?;
            }
        }
    }
    if retained.is_empty() || retained.len() == variables.len() {
        return Ok(None);
    }
    // A fixed guard on an interface variable stays in the residual body. Only
    // guards of truly local variables are consumed by the projected block.
    guards.retain(|(_, name, _)| !retained.contains(name));
    Ok(Some(Block {
        anchor,
        retained,
        guards,
        rows,
    }))
}

struct Selected<'p> {
    values: Vec<&'p Symbol>,
    witness: &'p [Term],
}

fn bindings<'a, 'p>(
    anchor: &'a Atom,
    row: &'p [Term],
    reading: &mut Reading<'_>,
) -> Result<Option<Vec<(&'a VarName, &'p Symbol)>>, Failure> {
    if terms(anchor).len() != row.len() {
        return Ok(None);
    }
    let mut bindings: Vec<(&'a VarName, &'p Symbol)> = Vec::new();
    for (pattern, value) in terms(anchor).iter().zip(row) {
        reading.step()?;
        let Term::Symbolic(value) = value else {
            return Ok(None);
        };
        if !atomic(value) {
            return Ok(None);
        }
        match pattern {
            Term::Symbolic(expected) if expected != value => return Ok(None),
            Term::Symbolic(_) => {}
            Term::Variable(Variable::Named(name)) => {
                if let Some((_, previous)) = bindings.iter().find(|(old, _)| *old == name) {
                    if *previous != value {
                        return Ok(None);
                    }
                } else {
                    reading.push(&mut bindings, (name, value))?;
                }
            }
            _ => unreachable!("flat anchor checked before matching"),
        }
    }
    Ok(Some(bindings))
}

fn project<'a, 'p>(
    literals: &[&Literal],
    block: &Block<'a, 'p>,
    maximum: usize,
    reading: &mut Reading<'_>,
) -> Result<Option<Vec<Selected<'p>>>, Failure> {
    let mut selected: Vec<Selected<'p>> = Vec::new();
    let mut witnesses = 0;
    // An unhandled row would make this a partial projection. Validate the
    // entire anchor carrier before treating a failed match as absence.
    for row in block.rows {
        for term in *row {
            reading.step()?;
            if !matches!(term, Term::Symbolic(value) if atomic(value)) {
                return Ok(None);
            }
        }
    }
    for row in block.rows {
        reading.step()?;
        let Some(bindings) = bindings(atom(literals[block.anchor]), row, reading)? else {
            continue;
        };
        let value = |name: &VarName| {
            bindings
                .iter()
                .find(|(variable, _)| *variable == name)
                .expect("anchor binds its variables")
                .1
        };
        let mut permitted = true;
        for (_, variable, rows) in &block.guards {
            let mut found = false;
            for row in *rows {
                reading.step()?;
                if let [Term::Symbolic(guard)] = *row
                    && guard == value(variable)
                {
                    found = true;
                    break;
                }
            }
            permitted &= found;
        }
        if !permitted {
            continue;
        }
        witnesses += 1;
        let mut duplicate = false;
        for previous in &selected {
            let mut same = true;
            for (variable, old) in block.retained.iter().zip(&previous.values) {
                reading.step()?;
                same &= value(variable) == *old;
            }
            if same {
                duplicate = true;
                break;
            }
        }
        if duplicate {
            continue;
        }
        if selected.len() == maximum {
            return Ok(None);
        }
        let mut values = Vec::new();
        for variable in &block.retained {
            reading.push(&mut values, value(variable))?;
        }
        reading.push(
            &mut selected,
            Selected {
                values,
                witness: row,
            },
        )?;
    }
    Ok((!selected.is_empty() && selected.len() < witnesses).then_some(selected))
}

fn family(
    literals: &[&Literal],
    facts: &mut FixedFacts<'_>,
    maximum: usize,
    reading: &mut Reading<'_>,
) -> Result<Option<Vec<Rule>>, Failure> {
    for anchor in 0..literals.len() {
        reading.step()?;
        let Some(variables) = anchor_variables(atom(literals[anchor]), reading)? else {
            continue;
        };
        // Only a flat anchor with two distinct named variables can project.
        // Its fixed rows still come from the same authoritative whole program.
        let index = facts.read(reading.work).map_err(|_| Failure::Optional)?;
        let Some(block) = block(literals, anchor, &variables, index, reading)? else {
            continue;
        };
        let Some(selected) = project(literals, &block, maximum, reading)? else {
            continue;
        };
        return emit(literals, &block, &selected, reading);
    }
    Ok(None)
}

/// Check the named retained and overlapping construction payload before
/// cloning a residual and building its typed substitution. This admits storage;
/// the later compiler and value constructors keep their own capacity checks.
fn admit_residual_storage(
    literals: &[&Literal],
    block: &Block<'_, '_>,
    selected: &Selected<'_>,
    bytes: u128,
    reading: &mut Reading<'_>,
) -> Result<Option<u128>, Failure> {
    let anchor = atom(literals[block.anchor]);
    // Bound copied source cells and substituted atomic text before calling
    // the dependency's total, iterative typed substitution operation.
    let mut payload = size_of::<Rule>() as u128;
    let max_value = selected
        .witness
        .iter()
        .filter_map(|term| {
            if let Term::Symbolic(value) = term {
                Some(crate::structural_value::symbol_bytes(value))
            } else {
                None
            }
        })
        .max()
        .unwrap_or(0);
    for (position, literal) in literals.iter().enumerate() {
        reading.step()?;
        if position == block.anchor || block.guards.iter().any(|(guard, _, _)| *guard == position) {
            continue;
        }
        let atom = atom(literal);
        payload = payload.saturating_add(
            size_of::<BodyElement>() as u128
                + size_of::<Atom>() as u128
                + atom.name.as_str().len() as u128,
        );
        for term in terms(atom).iter().flat_map(Term::subterms) {
            reading.step()?;
            payload = payload.saturating_add(
                size_of::<Term>() as u128
                    + match term {
                        Term::Symbolic(value) => crate::structural_value::symbol_bytes(value),
                        Term::Variable(Variable::Named(name)) => {
                            max_value + name.as_str().len() as u128
                        }
                        Term::Function { name, .. } => name.as_str().len() as u128,
                        _ => 0,
                    },
            );
        }
    }
    let anchor_bytes = terms(anchor).len() as u128
        * (size_of::<Term>() as u128 + max_value + size_of::<Variable>() as u128)
        + anchor.name.as_str().len() as u128;
    // The cloned residual and substitution's output overlap; the anchor
    // witness and unifier's finite argument/binding cells coexist with both.
    let temporary = payload
        .saturating_mul(2)
        .saturating_add(anchor_bytes.saturating_mul(4));
    if !reading.family(bytes.saturating_add(temporary))? {
        return Ok(None);
    }
    reading
        .budget
        .charge(ExpansionResource::ScalarBytes, temporary, reading.site)?;
    Ok(Some(bytes.saturating_add(payload)))
}

fn emit(
    literals: &[&Literal],
    block: &Block<'_, '_>,
    selected: &[Selected<'_>],
    reading: &mut Reading<'_>,
) -> Result<Option<Vec<Rule>>, Failure> {
    let mut rules = Vec::new();
    let mut bytes = 0_u128;
    for selected in selected {
        reading.step()?;
        let anchor = atom(literals[block.anchor]);
        let Some(admitted_bytes) =
            admit_residual_storage(literals, block, selected, bytes, reading)?
        else {
            return Ok(None);
        };
        bytes = admitted_bytes;
        let mut body = Vec::new();
        for (position, literal) in literals.iter().enumerate() {
            if position != block.anchor
                && !block.guards.iter().any(|(guard, _, _)| *guard == position)
            {
                reading.push(&mut body, BodyElement::Literal((*literal).clone()))?;
            }
        }
        let witness = Atom {
            sign: anchor.sign,
            name: anchor.name.clone(),
            arguments: Arguments::Single(selected.witness.to_vec()),
        };
        let substitution = mgu(anchor, &witness)
            .expect("flat atomic anchor is a pattern")
            .expect("selected matching witness");
        let rule = substitute(Rule::new(Head::Falsum, Body::new(body)), &substitution);
        // The pinned typed substitution canonicalizes every newly ground function
        // and tuple into one Symbolic value (including its entire subtree).
        // Validate that whole value before it can move admission earlier than
        // a support row. Logical refusal leaves the original rule intact;
        // fallible validation storage still reports its typed allocation fault.
        for element in rule.body().get().elements() {
            let BodyElement::Literal(literal) = element.get() else {
                unreachable!("positive residual")
            };
            for term in atom(literal).argument_terms().flat_map(Term::subterms) {
                reading.step()?;
                if let Term::Symbolic(value) = term {
                    match crate::compile::validate_scalar(value, reading.site) {
                        Ok(()) => {}
                        Err(
                            error @ crate::AdmissionFailure::Construction {
                                error:
                                    zetesis_core::ConstructionError::Value(
                                        zetesis_core::ValueError::Allocation,
                                    ),
                                ..
                            },
                        ) => return Err(Failure::Source(error.into())),
                        Err(_) => return Ok(None),
                    }
                }
            }
        }
        reading.push(&mut rules, rule)?;
    }
    Ok(Some(rules))
}

#[cfg(test)]
mod tests;
