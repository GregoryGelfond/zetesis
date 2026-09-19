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
//!
//! A key position must name its value in both patterns. The asked atom stands
//! under `not`, where `p(_, t)` holds when some key has the value `t`, so
//! `not p(_, t)` forbids only that no key has it, while the written constraint
//! forbids a wrong value at every key. A constraint with an anonymous key
//! position is left as written.
//!
//! The rewrite reads the normalized program, where facts are expanded and
//! constants resolved, and replaces the written constraint's source
//! statement. Every asked statement carries the written constraint's
//! provenance and the transformation's tag. Nothing is claimed for a
//! constraint outside the two patterns, which is left as written.

use std::collections::BTreeMap;

use themelios_base::span::Location;
use themelios_program::program::{
    Arguments, Atom, Body, BodyElement, DefaultNegation, Head, Literal, LiteralInner,
    Program as SourceProgram, Relation, Rule, Statement,
};
use themelios_program::provenance::{Origin, Provenance, TransformTag, WithProvenance};
use themelios_program::symbol::{Signature, Symbol, VarName};
use themelios_program::term::{BinaryOp, Term, Variable};
use zetesis_domain::{Analysis, Domain, Key, Status, atom_signature};

use crate::expansion::Budget;
use crate::formula_ir::Prepared;
use crate::{DomainLimits, ExpansionResource, FormulaFailure};

/// The program with its keyed constraints asked, and how many were.
pub(crate) struct Rewritten {
    pub program: SourceProgram,
    pub constraints: usize,
}

/// Ask every keyed constraint of the prepared program's source.
///
/// # Errors
/// Returns the budget refusal when the inspection exhausts the term work.
/// A stopped key analysis rewrites nothing.
pub(crate) fn rewrite(
    source: &SourceProgram,
    prepared: &Prepared,
    budget: &mut Budget,
    location: Location,
) -> Result<Option<Rewritten>, FormulaFailure> {
    let Ok(keys) = zetesis_domain::keys(&prepared.analyzed, &DomainLimits::default()) else {
        return Ok(None);
    };
    if keys.is_empty() {
        return Ok(None);
    }
    let keys: BTreeMap<&Signature, &Key<'_>> =
        keys.iter().map(|key| (key.signature(), key)).collect();
    // The normalized statement of each source statement, by parsed origin;
    // a source statement normalized into several is left as written.
    let mut normalized: BTreeMap<Location, Vec<&WithProvenance<Statement>>> = BTreeMap::new();
    for carrier in prepared.analyzed.statements() {
        budget.charge(ExpansionResource::TermWork, 1, location)?;
        if let [origin] = crate::extended::parsed_origins(carrier)[..] {
            normalized.entry(origin).or_default().push(carrier);
        }
    }
    let mut analysis = None;
    let mut statements = Vec::new();
    let mut constraints = 0;
    for carrier in source.statements() {
        budget.charge(ExpansionResource::TermWork, 1, location)?;
        let asked = match crate::extended::parsed_origins(carrier)[..] {
            [origin] => match normalized.get(&origin).map(Vec::as_slice) {
                Some([statement]) => ask(
                    statement,
                    &keys,
                    &prepared.analyzed,
                    &mut analysis,
                    budget,
                    location,
                )?,
                _ => None,
            },
            _ => None,
        };
        match asked {
            Some(rules) => {
                constraints += 1;
                let tag = Provenance::from(Origin::Transformed(TransformTag::new(
                    "zetesis-keyed-constraint",
                )));
                for rule in rules {
                    statements.push(WithProvenance::new(
                        Statement::Rule(rule),
                        carrier.provenance().clone().merge(tag.clone()),
                    ));
                }
            }
            None => statements.push(carrier.clone()),
        }
    }
    Ok((constraints > 0).then(|| Rewritten {
        program: SourceProgram::of_nodes(statements),
        constraints,
    }))
}

/// A positive atom of the constraint whose value position names a variable
/// read only there and in the comparison: the one the constraint demands.
struct Demand<'a> {
    element: usize,
    key: &'a Key<'a>,
    terms: &'a [Term],
    variable: &'a VarName,
}

/// The asked constraints for one written constraint, or `None` when it is
/// outside both patterns.
fn ask<'p>(
    statement: &WithProvenance<Statement>,
    keys: &BTreeMap<&Signature, &Key<'_>>,
    program: &'p SourceProgram,
    analysis: &mut Option<Analysis<'p>>,
    budget: &mut Budget,
    location: Location,
) -> Result<Option<Vec<Rule>>, FormulaFailure> {
    let Statement::Rule(rule) = statement.get() else {
        return Ok(None);
    };
    if !matches!(rule.head().get(), Head::Falsum) {
        return Ok(None);
    }
    let body = rule.body().get();
    let mut elements = Vec::new();
    let mut comparison = None;
    let mut occurrences: BTreeMap<&VarName, usize> = BTreeMap::new();
    for (index, element) in body.elements().enumerate() {
        budget.charge(ExpansionResource::TermWork, 1, location)?;
        let BodyElement::Literal(literal) = element.get() else {
            return Ok(None);
        };
        if literal.negation != DefaultNegation::None {
            return Ok(None);
        }
        match &literal.inner {
            LiteralInner::Atom(atom) => {
                let Arguments::Single(terms) = &atom.get().arguments else {
                    return Ok(None);
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
                    return Ok(None);
                };
                count(chain.first(), &mut occurrences);
                count(second, &mut occurrences);
                if relation == Relation::Neq {
                    if comparison.replace((index, chain.first(), second)).is_some() {
                        return Ok(None);
                    }
                } else {
                    elements.push(literal);
                }
            }
            LiteralInner::True | LiteralInner::False => return Ok(None),
        }
    }
    let Some((skipped, left, right)) = comparison else {
        return Ok(None);
    };
    let demands = demands(body, skipped, keys, &occurrences);
    let asked = one_value(left, right, &demands)
        .map(|(demand, value)| vec![(demand, value)])
        .or_else(|| digit_and_carry(left, right, &demands, program, analysis));
    let Some(asked) = asked else {
        return Ok(None);
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
        terms[demand.key.value()] = value.clone();
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
    Ok(Some(rules))
}

/// Every keyed atom of the body whose value is a variable read only there
/// and in the comparison: the values the constraint can demand.
fn demands<'a>(
    body: &'a Body,
    skipped: usize,
    keys: &BTreeMap<&Signature, &'a Key<'a>>,
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
        let Some(Term::Variable(Variable::Named(variable))) = terms.get(key.value()) else {
            continue;
        };
        // The asked atom stands under `not`, where an anonymous argument
        // reads as "for no value at all": the demanded atom is the key's one
        // atom only when every key position names its value.
        if occurrences.get(variable) != Some(&2)
            || terms.iter().enumerate().any(|(position, term)| {
                position != key.value() && (mentions(term, variable) || anonymous(term))
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
/// number by the analysis of the values their keys admit.
fn digit_and_carry<'a, 'p>(
    left: &Term,
    right: &Term,
    demands: &'a [Demand<'a>],
    program: &'p SourceProgram,
    analysis: &mut Option<Analysis<'p>>,
) -> Option<Vec<(&'a Demand<'a>, Term)>> {
    for (sum, other) in [(left, right), (right, left)] {
        let Some((digit, base, carry)) = digit_plus_carry(sum, demands) else {
            continue;
        };
        if mentions(other, digit.variable) || mentions(other, carry.variable) {
            continue;
        }
        let analysis = analysis
            .get_or_insert_with(|| zetesis_domain::analyze(program, DomainLimits::default()));
        if analysis.status() != Status::FixedPoint
            || !admits_within(analysis, digit.key, 0, i64::from(base) - 1)
            || !admits_within(analysis, carry.key, 0, i64::from(i32::MAX))
        {
            continue;
        }
        let quotient = |operator| Term::BinaryOperation {
            operator,
            left: Box::new(other.clone()),
            right: Box::new(Term::Symbolic(Symbol::Number(base))),
        };
        return Some(vec![
            (digit, quotient(BinaryOp::Mod)),
            (carry, quotient(BinaryOp::Div)),
        ]);
    }
    None
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

/// Whether every value the key's condition admits is a number in `low..=high`.
fn admits_within(analysis: &Analysis<'_>, key: &Key<'_>, low: i64, high: i64) -> bool {
    let mut positions = key.condition().literals().filter_map(|literal| {
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
    // The value is bound by the condition, so a literal names it; every one
    // that does bounds it, and one within range suffices.
    positions.any(|(signature, position)| within(analysis.domain(&signature, position), low, high))
}

/// Whether every value of the domain is a number in `low..=high`.
fn within(domain: &Domain<'_>, low: i64, high: i64) -> bool {
    match domain {
        Domain::Finite(values) => values.iter().all(|value| {
            matches!(value, Symbol::Number(number) if (low..=high).contains(&i64::from(*number)))
        }),
        Domain::Unknown => false,
    }
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
