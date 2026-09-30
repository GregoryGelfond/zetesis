//! Model-relative bounded joins. No source carrier or solver is consulted.

mod scopes;
mod patterns;
mod query;
mod rows;
mod values;
mod inverse;
mod anonymous;
mod work;
mod binding;
mod interpreter;
mod output;
#[cfg(test)]
mod test_support;

use super::{
    Binder, Cancellation, Condition, ConstructionLimits, DefaultNegation, Error, ErrorKind,
    Evaluation, EvaluationError, Limits, Model, ObservationProgram, Operand, Pattern, Query,
    Relation, Resource, Statistics, Symbol, Template,
};
use binding::Binding;
use interpreter::Interpreter;
use query::visit;
use rows::ModelRows;
use std::cmp::Ordering;
use themelios_program::term::UnaryOp;
pub(super) use work::Work;
use zetesis_core::catalog::{AtomRef, DerivedFailure, DerivedTerms, TermRef};

#[derive(Clone, Copy, Default)]
struct Metric {
    nodes: usize,
    bytes: usize,
}
impl Metric {
    fn construction_bytes(self) -> u128 {
        2 * self.nodes as u128 * size_of::<Symbol>() as u128 + 2 * self.bytes as u128
    }
    fn payload(self) -> u128 {
        self.nodes as u128 * 16 + self.bytes as u128
    }
}

fn matches<'input>(
    pattern: &Pattern,
    atom: AtomRef<'input>,
    binding: &mut Binding<'input>,
    undo: &mut Vec<usize>,
    bind: bool,
    ctx: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    let predicate = ctx
        .metadata
        .predicate(pattern.predicate)
        .expect("compiled predicate");
    if !atom
        .predicate()
        .equals_ref_with(predicate, || ctx.work.step(1))?
    {
        return Ok(false);
    }
    for (term, value) in pattern.terms.iter().zip(atom.values()) {
        if !patterns::matches_value(term, value, binding, undo, bind, ctx)? {
            return Ok(false);
        }
    }
    if pattern.evaluated && !test_pattern(pattern, atom, binding, ctx)? {
        return Ok(false);
    }
    if let Some(slot) = pattern.key {
        let (key, metric) = anonymous::capture(atom, ctx)?;
        ctx.work.check(
            Resource::LocalBytes,
            ctx.work.local_bytes + metric.payload(),
            ctx.work.limits.max_local_bytes as u128,
        )?;
        binding.bind_pattern(slot, key, metric, ctx.work);
        undo.push(slot);
    }
    Ok(true)
}
fn relation_holds(relation: Relation, order: Ordering) -> bool {
    match relation {
        Relation::Lt => order.is_lt(),
        Relation::Le => !order.is_gt(),
        Relation::Gt => order.is_gt(),
        Relation::Ge => !order.is_lt(),
        Relation::Eq => order.is_eq(),
        Relation::Neq => !order.is_eq(),
    }
}
fn test_pattern<'input>(
    pattern: &Pattern,
    atom: AtomRef<'input>,
    binding: &Binding<'input>,
    ctx: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    let predicate = ctx
        .metadata
        .predicate(pattern.predicate)
        .expect("compiled predicate");
    if !atom
        .predicate()
        .equals_ref_with(predicate, || ctx.work.step(1))?
    {
        return Ok(false);
    }
    for (term, value) in pattern.terms.iter().zip(atom.values()) {
        if !patterns::test_value(term, value, binding, ctx)? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn condition<'input>(
    condition: &Condition,
    atoms: &ModelRows<'input>,
    binding: &Binding<'input>,
    ctx: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    ctx.work.step(1)?;
    match condition {
        Condition::Atom(negation, alternatives) => {
            for alternative in alternatives {
                let predicate = ctx
                    .metadata
                    .predicate(alternative.pattern.predicate)
                    .expect("compiled predicate");
                let range = atoms.predicate(predicate, ctx.work)?;
                let test = |binding: &Binding<'input>, ctx: &mut Interpreter<'input, '_, '_>| {
                    for row in range.clone() {
                        if test_pattern(&alternative.pattern, atoms.get(row), binding, ctx)? {
                            return Ok(*negation != DefaultNegation::Not);
                        }
                    }
                    Ok(*negation == DefaultNegation::Not)
                };
                let truth = if alternative.expansion.binders.is_empty() {
                    test(binding, ctx)?
                } else {
                    !visit(
                        &alternative.expansion,
                        atoms,
                        Some(binding),
                        ctx,
                        &mut |local, ctx| test(local, ctx).map(|truth| !truth),
                    )?
                };
                if truth {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Condition::AtomPatternValue(negation, slot) => {
            let key = binding.pattern(*slot).expect("generated atom pattern key");
            let mut present = false;
            for row in atoms.predicate(anonymous::predicate(key, ctx), ctx.work)? {
                if anonymous::atom(key, atoms.get(row), ctx)? {
                    present = true;
                    break;
                }
            }
            Ok(present != (*negation == DefaultNegation::Not))
        }
        Condition::Compare(negation, first, steps) => {
            let mut left = first;
            let mut truth = true;
            for (relation, right) in steps {
                truth = relation_holds(*relation, ctx.compare_templates(left, right, binding)?);
                if !truth {
                    break;
                }
                left = right;
            }
            Ok(truth != (*negation == DefaultNegation::Not))
        }
        Condition::Boolean(truth) => Ok(*truth),
        Condition::Conditional(query, consequent) => {
            visit(query, atoms, Some(binding), ctx, &mut |local, ctx| {
                self::condition(consequent, atoms, local, ctx)
            })
        }
        Condition::Aggregate(negation, aggregate, guards) => {
            let (value, metric) = scopes::aggregate(aggregate, atoms, binding, ctx)?;
            let mut truth = true;
            for guard in guards {
                let mut bound_metric = Metric::default();
                ctx.measure(&guard.bound, binding, 1, &mut bound_metric)?;
                ctx.work.construction_check(Metric {
                    nodes: metric.nodes + bound_metric.nodes,
                    bytes: metric.bytes + bound_metric.bytes,
                })?;
                let bound = ctx.construct(&guard.bound, binding)?;
                if !relation_holds(guard.relation, value.compare(&bound, ctx)?) {
                    truth = false;
                    break;
                }
            }
            Ok(truth != (*negation == DefaultNegation::Not))
        }
    }
}
fn conditions<'input>(
    query: &Query,
    atoms: &ModelRows<'input>,
    binding: &Binding<'input>,
    ctx: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    for test in &query.conditions {
        if !condition(test, atoms, binding, ctx)? {
            return Ok(false);
        }
    }
    Ok(true)
}
type Visitor<'input, 'visitor> =
    dyn FnMut(&Binding<'input>, &mut Interpreter<'input, '_, '_>) -> Result<bool, Error> + 'visitor;
fn complete<'input>(
    query: &Query,
    atoms: &ModelRows<'input>,
    binding: &mut Binding<'input>,
    ctx: &mut Interpreter<'input, '_, '_>,
    visitor: &mut Visitor<'input, '_>,
) -> Result<bool, Error> {
    ctx.work.step(1)?;
    ctx.work.check(
        Resource::Bindings,
        u128::from(ctx.work.statistics.bindings) + 1,
        u128::from(ctx.work.limits.max_bindings),
    )?;
    ctx.work.statistics.bindings += 1;
    if conditions(query, atoms, binding, ctx)? {
        visitor(binding, ctx)
    } else {
        Ok(true)
    }
}
pub(super) fn terms(
    program: &ObservationProgram,
    model: &Model,
    work: &mut Work<'_>,
) -> Result<Vec<Symbol>, Error> {
    work.step(0)?;
    let Some(program) = program
        .read_with(|| work.step(1))
        .map_err(|error| match error {
            zetesis_core::TemplateCatalogFailure::Stopped(error) => error,
            zetesis_core::TemplateCatalogFailure::Read(error) => {
                work.error(ErrorKind::TermAssignment(error.into()))
            }
            zetesis_core::TemplateCatalogFailure::Storage(error) => {
                work.error(ErrorKind::TermStorage(error))
            }
            zetesis_core::TemplateCatalogFailure::Incomplete => {
                unreachable!("immutable compiled metadata is complete")
            }
        })?
    else {
        return Ok(Vec::new());
    };
    let atoms = ModelRows::new(model, work)?;
    let terms = DerivedTerms::new_with(
        &[program.metadata.catalog(), model.catalog().read()],
        work.limits.max_term_storage_bytes,
        || work.step(1),
    )
    .map_err(|error| match error {
        DerivedFailure::Storage(zetesis_core::catalog::Error::Storage { required, .. }) => work
            .error(ErrorKind::Limit {
                resource: Resource::TermStorageBytes,
                observed: required,
                limit: work.limits.max_term_storage_bytes as u128,
            }),
        DerivedFailure::Storage(error) => work.error(ErrorKind::TermStorage(error)),
        DerivedFailure::Read(error) => work.error(ErrorKind::TermAssignment(error.into())),
        DerivedFailure::Assignment(error) => work.error(ErrorKind::TermAssignment(error)),
        DerivedFailure::Stopped(error) => error,
    })?;
    let keys = anonymous::Keys::new(terms.read());
    let mut ctx = Interpreter {
        metadata: program.metadata,
        terms,
        work,
        keys,
    };
    ctx.observe();
    let result = (|| {
        ctx.work.check(
            Resource::TermStorageBytes,
            ctx.work.statistics.term_storage_bytes,
            ctx.work.limits.max_term_storage_bytes as u128,
        )?;
        let mut result = output::Terms::new(ctx.terms.read());
        for directive in program.directives {
            ctx.work.location = directive.origins.first().copied();
            visit(
                &directive.query,
                &atoms,
                None,
                &mut ctx,
                &mut |binding, ctx| {
                    values::each(&directive.term, binding, ctx, |term, metric, ctx| {
                        result.insert(&term, metric, ctx)
                    })?;
                    Ok(true)
                },
            )?;
        }
        result.export(&mut ctx)
    })();
    ctx.observe();
    result.map_err(|mut error| {
        error.statistics = ctx.work.statistics;
        error
    })
}
pub(super) fn evaluate(
    program: &ObservationProgram,
    model: &Model,
    limits: Limits,
    construction: ConstructionLimits,
    cancellation: &Cancellation,
) -> Result<Evaluation, Error> {
    let mut work = Work {
        limits,
        construction,
        cancellation,
        statistics: Statistics::default(),
        local_bytes: 0,
        location: None,
    };
    let symbols = terms(program, model, &mut work)?;
    Ok(Evaluation {
        symbols,
        statistics: work.statistics,
    })
}
