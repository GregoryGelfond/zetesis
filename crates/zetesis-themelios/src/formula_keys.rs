//! Constraints over a keyed value, asked as the one atom the key admits.
//!
//! A keyed relation holds exactly one value per key ([`zetesis_domain::keys`]).
//! A constraint that reads that value only to compare it therefore forbids
//! exactly the answer sets in which the key's one value is not the demanded
//! one, and can ask for the demanded atom instead of reading every value:
//! the product of the demanded value with every value the key admits is
//! never formed. Two patterns are read; each carries its meaning argument.
//!
//! *One value.* `:- G, p(k, Y), Y != t.`, with `Y` read nowhere else, `t`
//! free of `Y` and no anonymous variable in the key `k`, becomes `:- G, b(k), not p(k, t).`, where `b` is the key's
//! body. Whenever `b(k)` holds, exactly one `p(k, y)` holds; the written
//! constraint fires iff that `y` differs from `t`, iff `p(k, t)` is absent,
//! iff the asked constraint fires. Whenever `b(k)` does not hold, no `p(k, _)`
//! holds and neither constraint fires.
//!
//! *A digit and a carry.* `:- G, p(k, Y), q(j, C), s != Y + c*C.`, with `Y`
//! and `C` read nowhere else, `s` free of both, `c ≥ 2`, every value `p`
//! admits in `0..c-1` and every value `q` admits a natural number, becomes
//! `:- G, b(k), b'(j), not p(k, s \ c).` and `:- G, b(k), b'(j), not q(j, s / c).`
//! For `s ≥ 0` the equation `s = y + c·C` over those ranges has the one
//! solution `y = s \ c`, `C = s / c`; the written constraint fires iff the
//! chosen pair is not that solution, iff one of the two asked atoms is
//! absent. For `s < 0` no pair in range solves it and the written constraint
//! fires; `s \ c` is then negative or zero with `s / c` negative, so an asked
//! atom is absent and an asked constraint fires. Division by `c ≥ 2` is
//! defined, and `s` is evaluated where the written constraint evaluated it.
//! The values `p` admits are those of its choice's condition, and the
//! condition's are read from the facts of a literal binding the value when
//! facts are all that produces it ([`zetesis_domain::facts`]): what the facts
//! admit is exactly what the relation admits. A condition produced otherwise
//! bounds nothing, and the constraint is left as written.
//!
//! Both patterns also require a checked-evaluation certificate for every
//! term of the original constraint. Independent fact bounds must establish
//! numeric operands and an `i32` range at every arithmetic intermediate;
//! comparisons supply no such bounds. This includes the retained body, since
//! replacing a comparison by a negative gate changes which substitutions
//! are excluded. The digit-and-carry pattern additionally requires `s` to be
//! numeric. Unproved safety leaves the constraint written, preserving its
//! reached failures, exclusions and diagnostics. The mathematical integer
//! equation alone does not establish these machine-arithmetic obligations.
//!
//! A key position must name its value in both patterns. The asked atom stands
//! under `not`, where `p(_, t)` holds when some key has the value `t`, so
//! `not p(_, t)` forbids only that no key has it, while the written constraint
//! forbids a wrong value at every key. A constraint with an anonymous key
//! position is left as written.
//!
//! The rewrite reads the analyzed program, where facts are expanded and
//! constants resolved, once every statement is compiled, and names the
//! written constraints to replace by their parsed origin; preparation then
//! compiles the asked constraints in their place, so nothing is prepared
//! twice. The key analysis and its readings of facts run under the key work
//! ceiling and the preparation's remaining term work, and their steps are
//! charged to the term work; a stop ends the asking, leaves every constraint
//! not yet asked as written, and is reported. Nothing is claimed for a
//! constraint outside the two patterns, which is left as written.
//!
//! The analyzed program may be a dependency projection
//! ([`crate::AnalysisBasis`]): a statement with a pool that outlives
//! normalization, such as one inside a condition or an aggregate element,
//! stands in it as one pool-free copy per alternative, read as several
//! statements where the program means one. The rewrite is sound over it. A
//! key is claimed only for a relation with one producer, and every copy of
//! a projected statement produces what the statement produces, since a pool
//! never changes an atom's signature and a pooled argument list is unpooled
//! before the projection, into the elements of its choice or into whole
//! rules; so a projected statement, of two copies at least, is never a
//! key's producer, and a projected constraint, several statements under one
//! origin, is never asked. Copies that coincide come from coinciding
//! alternatives, which the choice reads as one element.

use std::collections::BTreeMap;

use themelios_base::span::Location;
use themelios_program::program::{
    Arguments, Atom, Body, BodyElement, DefaultNegation, Head, Literal, LiteralInner,
    Program as SourceProgram, Relation, Rule, Statement,
};
use themelios_program::provenance::{Provenance, WithProvenance};
use themelios_program::symbol::{Signature, Symbol, VarName};
use themelios_program::term::{BinaryOp, Term, Variable};
use zetesis_domain::{KeyWork, KeyedRelation, Stop, atom_signature};

use crate::expansion::Budget;
use crate::{ExpansionResource, FormulaFailure, FormulaLimits};

mod safety;

/// How the key analysis of one preparation ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAnalysis {
    /// Every keyed relation was read, and every constraint in one of the two
    /// patterns asked.
    Complete,
    /// The analysis exceeded its work ceiling: every constraint not yet
    /// asked was left as written, which changes no answer set.
    Stopped(Stop),
}

/// The constraints asked in place of written ones, by the parsed origin of
/// the written constraint each set replaces, with that constraint's
/// provenance; and how the key analysis ended.
pub(crate) struct Asked {
    pub rules: BTreeMap<Location, (Provenance, Vec<Rule>)>,
    pub analysis: KeyAnalysis,
}

/// What one written constraint became.
enum Outcome {
    Asked(Vec<Rule>),
    Written,
    Stopped(Stop),
}

/// Ask every keyed constraint of the analyzed program. The key analysis and
/// every reading of facts spend at most `limits.max_key_work` steps and no
/// more than the term work remaining, and the steps spent are charged to
/// the term work whether the analysis completed or stopped.
///
/// # Errors
/// Returns the budget refusal when the inspection of a statement exhausts
/// the term work.
pub(crate) fn ask_all(
    analyzed: &SourceProgram,
    limits: &FormulaLimits,
    budget: &mut Budget,
    location: Location,
) -> Result<Asked, FormulaFailure> {
    let mut work = KeyWork::new(limits.max_key_work.min(budget.remaining_term_work()));
    let asked = ask_under(analyzed, &mut work, budget, location);
    budget.charge(
        ExpansionResource::TermWork,
        u128::from(work.steps()),
        location,
    )?;
    asked
}

fn ask_under(
    analyzed: &SourceProgram,
    work: &mut KeyWork,
    budget: &mut Budget,
    location: Location,
) -> Result<Asked, FormulaFailure> {
    let mut asked = Asked {
        rules: BTreeMap::new(),
        analysis: KeyAnalysis::Complete,
    };
    let keys = match zetesis_domain::keys(analyzed, work) {
        Ok(keys) => keys,
        Err(stop) => {
            asked.analysis = KeyAnalysis::Stopped(stop);
            return Ok(asked);
        }
    };
    if keys.is_empty() {
        return Ok(asked);
    }
    let keys: BTreeMap<&Signature, &KeyedRelation<'_>> =
        keys.iter().map(|key| (key.signature(), key)).collect();
    // The analyzed statement of each source statement, by parsed origin; a
    // source statement normalized into several is left as written.
    let mut by_origin: BTreeMap<Location, Vec<&WithProvenance<Statement>>> = BTreeMap::new();
    for carrier in analyzed.statements() {
        budget.charge(ExpansionResource::TermWork, 1, location)?;
        if let [origin] = crate::extended::parsed_origins(carrier)[..] {
            by_origin.entry(origin).or_default().push(carrier);
        }
    }
    for (origin, statements) in by_origin {
        let [statement] = statements[..] else {
            continue;
        };
        match ask(statement, &keys, analyzed, work, budget, location)? {
            Outcome::Asked(rules) => {
                asked
                    .rules
                    .insert(origin, (statement.provenance().clone(), rules));
            }
            Outcome::Written => {}
            Outcome::Stopped(stop) => {
                asked.analysis = KeyAnalysis::Stopped(stop);
                return Ok(asked);
            }
        }
    }
    Ok(asked)
}

/// A positive atom of the constraint whose value position names a variable
/// read only there and in the comparison: the one the constraint demands.
struct Demand<'a> {
    element: usize,
    key: &'a KeyedRelation<'a>,
    terms: &'a [Term],
    variable: &'a VarName,
}

/// The asked constraints for one written constraint, or `None` when it is
/// outside both patterns.
fn ask(
    statement: &WithProvenance<Statement>,
    keys: &BTreeMap<&Signature, &KeyedRelation<'_>>,
    program: &SourceProgram,
    work: &mut KeyWork,
    budget: &mut Budget,
    location: Location,
) -> Result<Outcome, FormulaFailure> {
    let Statement::Rule(rule) = statement.get() else {
        return Ok(Outcome::Written);
    };
    if !matches!(rule.head().get(), Head::Falsum) {
        return Ok(Outcome::Written);
    }
    let body = rule.body().get();
    let mut elements = Vec::new();
    let mut comparison = None;
    let mut occurrences: BTreeMap<&VarName, usize> = BTreeMap::new();
    for (index, element) in body.elements().enumerate() {
        budget.charge(ExpansionResource::TermWork, 1, location)?;
        let BodyElement::Literal(literal) = element.get() else {
            return Ok(Outcome::Written);
        };
        if literal.negation != DefaultNegation::None {
            return Ok(Outcome::Written);
        }
        match &literal.inner {
            LiteralInner::Atom(atom) => {
                let Arguments::Single(terms) = &atom.get().arguments else {
                    return Ok(Outcome::Written);
                };
                for term in terms {
                    count(term, &mut occurrences);
                }
                elements.push(literal);
            }
            LiteralInner::Comparison(chain) => {
                let chain = chain.get();
                let mut steps = chain.steps();
                let (Some((relation, second)), None) = (steps.next(), steps.next()) else {
                    return Ok(Outcome::Written);
                };
                count(chain.first(), &mut occurrences);
                count(second, &mut occurrences);
                if relation == Relation::Neq {
                    if comparison.replace((index, chain.first(), second)).is_some() {
                        return Ok(Outcome::Written);
                    }
                } else {
                    elements.push(literal);
                }
            }
            LiteralInner::True | LiteralInner::False => return Ok(Outcome::Written),
        }
    }
    let Some((skipped, left, right)) = comparison else {
        return Ok(Outcome::Written);
    };
    let proof = match safety::Proof::of(body, keys, program, work) {
        Ok(Some(proof)) => proof,
        Ok(None) => return Ok(Outcome::Written),
        Err(failure) => return applicability_failure(failure, location),
    };
    let demands = demands(body, skipped, keys, &occurrences);
    let asked = match one_value(left, right, &demands) {
        Some((demand, value)) => vec![(demand, value)],
        None => match digit_and_carry(left, right, &demands, program, &proof, work) {
            Ok(Some(asked)) => asked,
            Ok(None) => return Ok(Outcome::Written),
            Err(failure) => return applicability_failure(failure, location),
        },
    };
    let rest: Vec<&Literal> = body
        .elements()
        .enumerate()
        .filter(|(index, _)| {
            *index != skipped && asked.iter().all(|(demand, _)| demand.element != *index)
        })
        .filter_map(|(_, element)| match element.get() {
            BodyElement::Literal(literal) => Some(literal),
            _ => None,
        })
        .collect();
    let mut rules = Vec::new();
    for (demand, value) in &asked {
        let mut literals: Vec<Literal> = rest.iter().map(|literal| (*literal).clone()).collect();
        for (demand, _) in &asked {
            literals.extend(key_body(demand));
        }
        let mut terms = demand.terms.to_vec();
        terms[demand.key.value_position()] = value.clone();
        literals.push(Literal {
            negation: DefaultNegation::Not,
            inner: LiteralInner::Atom(WithProvenance::constructed(Atom {
                sign: demand.key.signature().sign,
                name: demand.key.signature().name.clone(),
                arguments: Arguments::Single(terms),
            })),
        });
        rules.push(Rule::new(
            Head::Falsum,
            Body::new(literals.into_iter().map(BodyElement::Literal)),
        ));
    }
    Ok(Outcome::Asked(rules))
}

/// Exhausted key work declines rewriting; a failed allocation is a located
/// resource refusal. Both applicability checks use the same boundary.
fn applicability_failure(
    failure: safety::Failure,
    location: Location,
) -> Result<Outcome, FormulaFailure> {
    match failure {
        safety::Failure::Work(stop) => Ok(Outcome::Stopped(stop)),
        safety::Failure::Allocation => Err(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Allocation,
            location,
        }),
    }
}

/// Every keyed atom of the body whose value is a variable read only there
/// and in the comparison: the values the constraint can demand.
fn demands<'a>(
    body: &'a Body,
    skipped: usize,
    keys: &BTreeMap<&Signature, &'a KeyedRelation<'a>>,
    occurrences: &BTreeMap<&VarName, usize>,
) -> Vec<Demand<'a>> {
    let mut demands = Vec::new();
    for (index, literal) in body.elements().enumerate() {
        if index == skipped {
            continue;
        }
        let BodyElement::Literal(Literal {
            inner: LiteralInner::Atom(atom),
            ..
        }) = literal.get()
        else {
            continue;
        };
        let atom = atom.get();
        let Arguments::Single(terms) = &atom.arguments else {
            continue;
        };
        let Some(signature) = atom_signature(atom, terms.len()) else {
            continue;
        };
        let Some(key) = keys.get(&signature) else {
            continue;
        };
        let Some(Term::Variable(Variable::Named(variable))) = terms.get(key.value_position())
        else {
            continue;
        };
        // The asked atom stands under `not`, where an anonymous argument
        // reads as "for no value at all": the demanded atom is the key's one
        // atom only when every key position names its value.
        if occurrences.get(variable) != Some(&2)
            || terms.iter().enumerate().any(|(position, term)| {
                position != key.value_position() && (mentions(term, variable) || anonymous(term))
            })
        {
            continue;
        }
        demands.push(Demand {
            element: index,
            key,
            terms,
            variable,
        });
    }
    demands
}

/// `Y != t` or `t != Y` with `Y` demanded and `t` free of `Y`.
fn one_value<'a>(
    left: &Term,
    right: &Term,
    demands: &'a [Demand<'a>],
) -> Option<(&'a Demand<'a>, Term)> {
    for (side, other) in [(left, right), (right, left)] {
        let Term::Variable(Variable::Named(variable)) = side else {
            continue;
        };
        if let Some(demand) = demands.iter().find(|demand| demand.variable == variable)
            && !mentions(other, variable)
        {
            return Some((demand, other.clone()));
        }
    }
    None
}

/// `s != Y + c*C` in any order of sides and summands, with `Y` and `C`
/// demanded, `s` free of both, `Y` a digit in base `c` and `C` a natural
/// number by the facts of the conditions binding them.
///
/// # Errors
/// Returns a key work stop or a failed reservation for the safety proof.
fn digit_and_carry<'a>(
    left: &Term,
    right: &Term,
    demands: &'a [Demand<'a>],
    program: &SourceProgram,
    proof: &safety::Proof<'_>,
    work: &mut KeyWork,
) -> Result<Option<Vec<(&'a Demand<'a>, Term)>>, safety::Failure> {
    for (sum, other) in [(left, right), (right, left)] {
        let Some((digit, base, carry)) = digit_plus_carry(sum, demands) else {
            continue;
        };
        if mentions(other, digit.variable) || mentions(other, carry.variable) {
            continue;
        }
        // Source comparison accepts values of different kinds; the quotient
        // and remainder introduced here require a numeric dividend.
        if !proof.numeric(other, work)? {
            continue;
        }
        if !admits_within(program, digit.key, 0, i64::from(base) - 1, work)?
            || !admits_within(program, carry.key, 0, i64::from(i32::MAX), work)?
        {
            continue;
        }
        let quotient = |operator| Term::BinaryOperation {
            operator,
            left: Box::new(other.clone()),
            right: Box::new(Term::Symbolic(Symbol::Number(base))),
        };
        return Ok(Some(vec![
            (digit, quotient(BinaryOp::Mod)),
            (carry, quotient(BinaryOp::Div)),
        ]));
    }
    Ok(None)
}

/// `Y + c*C`, `c*C + Y`, `Y + C*c` or `C*c + Y` over two distinct demands.
fn digit_plus_carry<'a>(
    term: &Term,
    demands: &'a [Demand<'a>],
) -> Option<(&'a Demand<'a>, i32, &'a Demand<'a>)> {
    let Term::BinaryOperation {
        operator: BinaryOp::Add,
        left,
        right,
    } = term
    else {
        return None;
    };
    for (digit, product) in [(left, right), (right, left)] {
        let Term::Variable(Variable::Named(variable)) = &**digit else {
            continue;
        };
        let Some(digit) = demands.iter().find(|demand| demand.variable == variable) else {
            continue;
        };
        let Term::BinaryOperation {
            operator: BinaryOp::Mul,
            left,
            right,
        } = &**product
        else {
            continue;
        };
        for (base, carry) in [(left, right), (right, left)] {
            let Term::Symbolic(Symbol::Number(base)) = &**base else {
                continue;
            };
            let Term::Variable(Variable::Named(variable)) = &**carry else {
                continue;
            };
            let Some(carry) = demands.iter().find(|demand| demand.variable == variable) else {
                continue;
            };
            if *base >= 2 && !std::ptr::eq(digit, carry) {
                return Some((digit, *base, carry));
            }
        }
    }
    None
}

/// Whether every value the key's condition admits is a number in
/// `low..=high`, read from the facts of a literal of the condition binding
/// the value. The value is bound by the condition, so a literal names it;
/// every one that does bounds it, and one within range suffices. A literal
/// over a predicate produced by anything but facts bounds nothing.
///
/// # Errors
/// Returns the key work's stop.
fn admits_within(
    program: &SourceProgram,
    key: &KeyedRelation<'_>,
    low: i64,
    high: i64,
    work: &mut KeyWork,
) -> Result<bool, Stop> {
    let positions = key.condition().literals().filter_map(|literal| {
        let LiteralInner::Atom(atom) = &literal.get().inner else {
            return None;
        };
        let atom = atom.get();
        let Arguments::Single(terms) = &atom.arguments else {
            return None;
        };
        let position = terms
            .iter()
            .position(|term| mentions(term, key.value_variable()))?;
        Some((atom_signature(atom, terms.len())?, position))
    });
    for (signature, position) in positions {
        if let Some(values) = zetesis_domain::facts(program, &signature, position, work)?
            && within(&values, low, high)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether every value is a number in `low..=high`.
fn within(values: &[&Symbol], low: i64, high: i64) -> bool {
    values.iter().all(|value| {
        matches!(value, Symbol::Number(number) if (low..=high).contains(&i64::from(*number)))
    })
}

/// The key's body with its key variables replaced by the demand's terms.
/// The key's analysis admits a body of positive plain atoms with one
/// argument tuple each, so no other literal occurs here.
fn key_body(demand: &Demand<'_>) -> Vec<Literal> {
    let substitution: BTreeMap<&VarName, &Term> = demand
        .terms
        .iter()
        .enumerate()
        .filter_map(|(position, term)| Some((demand.key.key_variable(position)?, term)))
        .collect();
    demand
        .key
        .body()
        .elements()
        .map(|element| match element.get() {
            BodyElement::Literal(Literal {
                negation: DefaultNegation::None,
                inner: LiteralInner::Atom(atom),
            }) => {
                let atom = atom.get();
                let Arguments::Single(terms) = &atom.arguments else {
                    unreachable!("a key's body atom has one argument tuple")
                };
                Literal {
                    negation: DefaultNegation::None,
                    inner: LiteralInner::Atom(WithProvenance::constructed(Atom {
                        sign: atom.sign,
                        name: atom.name.clone(),
                        arguments: Arguments::Single(
                            terms
                                .iter()
                                .map(|term| match term {
                                    Term::Variable(Variable::Named(name)) => substitution
                                        .get(name)
                                        .map_or_else(|| term.clone(), |term| (*term).clone()),
                                    _ => term.clone(),
                                })
                                .collect(),
                        ),
                    })),
                }
            }
            _ => unreachable!("a key's body holds only positive plain atoms"),
        })
        .collect()
}

fn count<'a>(term: &'a Term, occurrences: &mut BTreeMap<&'a VarName, usize>) {
    for subterm in term.subterms() {
        if let Term::Variable(Variable::Named(name)) = subterm {
            *occurrences.entry(name).or_default() += 1;
        }
    }
}

fn anonymous(term: &Term) -> bool {
    term.subterms()
        .any(|subterm| matches!(subterm, Term::Variable(Variable::Anonymous)))
}

fn mentions(term: &Term, variable: &VarName) -> bool {
    term.subterms()
        .any(|subterm| matches!(subterm, Term::Variable(Variable::Named(name)) if name == variable))
}
