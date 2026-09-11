//! Model-relative bounded joins. No source carrier or solver is consulted.

mod scopes;
mod patterns;

use std::cmp::Ordering;

use themelios_base::span::Location;
use themelios_program::symbol::{Name, Sign};
use themelios_program::term::UnaryOp;
use zetesis_core::Atom;

use super::{
    Binder, Condition, ConstructionLimits, Control, DefaultNegation, Error, ErrorKind, Evaluation,
    EvaluationError, Limits, Model, ObservationProgram, Operand, Pattern, Query, Relation,
    Resource, Statistics, Symbol, Template, Value,
};

pub(super) struct Work<'a> {
    pub limits: Limits,
    pub construction: ConstructionLimits,
    pub control: &'a Control,
    pub statistics: Statistics,
    pub location: Option<Location>,
    pub local_bytes: u128,
}
impl Work<'_> {
    pub fn error(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            location: self.location,
            statistics: self.statistics,
        }
    }
    pub fn check(&self, resource: Resource, observed: u128, limit: u128) -> Result<(), Error> {
        if observed > limit {
            return Err(self.error(ErrorKind::Limit {
                resource,
                observed,
                limit,
            }));
        }
        Ok(())
    }
    pub fn step(&mut self, count: u128) -> Result<(), Error> {
        self.control
            .poll()
            .map_err(|stop| self.error(ErrorKind::Stopped(stop)))?;
        let observed = u128::from(self.statistics.work) + count;
        self.check(Resource::Work, observed, u128::from(self.limits.max_work))?;
        self.statistics.work = u64::try_from(observed).expect("work checked against u64 ceiling");
        Ok(())
    }
    pub fn reserve<T>(&self, count: usize) -> Result<Vec<T>, Error> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| self.error(ErrorKind::Allocation))?;
        Ok(values)
    }
    fn compare(&mut self, left: &Value, right: &Value) -> Result<Ordering, Error> {
        self.step(value_work(left) + value_work(right))?;
        Ok(left.compare_terms(right))
    }
    fn numeric(&mut self, term: &Template, binding: &[Option<Bound<'_>>]) -> Result<i32, Error> {
        self.step(1)?;
        let result = match term {
            Template::Value(Symbol::Number(value)) => Ok(*value),
            Template::Variable(slot) => match binding[*slot].as_ref().map(Bound::borrow) {
                Some(
                    Reference::Value(Value::Number(value))
                    | Reference::Symbol(Symbol::Number(value)),
                ) => Ok(*value),
                _ => Err(EvaluationError::Undefined),
            },
            Template::Unary(operator, argument) => {
                let value = self.numeric(argument, binding)?;
                crate::scalar_arithmetic::unary(*operator, value)
            }
            Template::Binary(operator, left, right) => {
                let left = self.numeric(left, binding)?;
                let right = self.numeric(right, binding)?;
                crate::scalar_arithmetic::binary(*operator, left, right)
            }
            Template::Absolute(argument) => {
                let value = self.numeric(argument, binding)?;
                crate::scalar_arithmetic::absolute(value)
            }
            _ => Err(EvaluationError::Undefined),
        };
        result.map_err(|cause| self.error(ErrorKind::Evaluation(cause)))
    }
    fn construction_check(&self, metric: Metric) -> Result<(), Error> {
        let construction = 2 * metric.nodes as u128 * std::mem::size_of::<Symbol>() as u128
            + 2 * metric.bytes as u128;
        self.check(
            Resource::ConstructionBytes,
            construction,
            self.construction.max_bytes as u128,
        )
    }
    fn compare_templates(
        &mut self,
        left: &Template,
        right: &Template,
        binding: &[Option<Bound<'_>>],
    ) -> Result<Ordering, Error> {
        let mut left_metric = Metric::default();
        let mut right_metric = Metric::default();
        self.measure(left, binding, 1, &mut left_metric)?;
        self.measure(right, binding, 1, &mut right_metric)?;
        self.construction_check(Metric {
            nodes: left_metric.nodes + right_metric.nodes,
            bytes: left_metric.bytes + right_metric.bytes,
        })?;
        let left = self.construct(left, binding)?;
        let right = self.construct(right, binding)?;
        self.step(left_metric.payload() + right_metric.payload())?;
        Ok(left.cmp(&right))
    }
    fn measure_reference(
        &mut self,
        value: Reference<'_>,
        depth: usize,
        metric: &mut Metric,
    ) -> Result<(), Error> {
        match value {
            Reference::Symbol(value) => self.symbol_check(value, depth, metric),
            Reference::Value(value) => {
                let count = if let Value::Structured(value) = value {
                    self.check(
                        Resource::Depth,
                        depth as u128 + value.depth() as u128 - 1,
                        self.limits.max_symbol_depth as u128,
                    )?;
                    self.step(value.nodes().len() as u128)?;
                    value.nodes().len()
                } else {
                    1
                };
                self.check(
                    Resource::Nodes,
                    metric.nodes as u128 + count as u128,
                    self.limits.max_symbol_nodes as u128,
                )?;
                metric.nodes += count;
                self.payload(scalar_bytes(value), metric)
            }
        }
    }
    fn copy_reference(&mut self, value: Reference<'_>) -> Result<Symbol, Error> {
        match value {
            Reference::Symbol(value) => self.copy_symbol(value),
            Reference::Value(value) => scalar(value).map_err(|kind| self.error(kind)),
        }
    }
    fn compare_reference(
        &mut self,
        left: Reference<'_>,
        right: Reference<'_>,
    ) -> Result<Ordering, Error> {
        if let (Reference::Value(left), Reference::Value(right)) = (left, right) {
            return self.compare(left, right);
        }
        let mut left_metric = Metric::default();
        let mut right_metric = Metric::default();
        self.measure_reference(left, 1, &mut left_metric)?;
        self.measure_reference(right, 1, &mut right_metric)?;
        let metric = Metric {
            nodes: left_metric.nodes + right_metric.nodes,
            bytes: left_metric.bytes + right_metric.bytes,
        };
        self.construction_check(metric)?;
        let left = self.copy_reference(left)?;
        let right = self.copy_reference(right)?;
        self.step(metric.payload())?;
        Ok(left.cmp(&right))
    }
    fn own(
        &mut self,
        term: &Template,
        binding: &[Option<Bound<'_>>],
    ) -> Result<(Symbol, Metric), Error> {
        let mut metric = Metric::default();
        self.measure(term, binding, 1, &mut metric)?;
        self.construction_check(metric)?;
        self.check(
            Resource::LocalBytes,
            self.local_bytes + metric.payload(),
            self.limits.max_local_bytes as u128,
        )?;
        let symbol = self.construct(term, binding)?;
        self.local_bytes += metric.payload();
        Ok((symbol, metric))
    }
    fn symbol_check(
        &mut self,
        symbol: &Symbol,
        depth: usize,
        metric: &mut Metric,
    ) -> Result<(), Error> {
        self.step(1)?;
        metric.nodes += 1;
        self.check(
            Resource::Nodes,
            metric.nodes as u128,
            self.limits.max_symbol_nodes as u128,
        )?;
        self.check(
            Resource::Depth,
            depth as u128,
            self.limits.max_symbol_depth as u128,
        )?;
        match symbol {
            Symbol::String(value) => self.payload(value.len(), metric)?,
            Symbol::Function {
                name, arguments, ..
            } => {
                self.payload(name.as_str().len(), metric)?;
                for argument in arguments {
                    self.symbol_check(argument, depth + 1, metric)?;
                }
            }
            Symbol::Tuple(arguments) => {
                for argument in arguments {
                    self.symbol_check(argument, depth + 1, metric)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn payload(&mut self, bytes: usize, metric: &mut Metric) -> Result<(), Error> {
        self.step(bytes as u128)?;
        let observed = metric.bytes as u128 + bytes as u128;
        self.check(
            Resource::Bytes,
            observed,
            self.limits.max_symbol_bytes as u128,
        )?;
        metric.bytes = usize::try_from(observed).expect("checked usize ceiling");
        Ok(())
    }
    fn measure(
        &mut self,
        term: &Template,
        binding: &[Option<Bound<'_>>],
        depth: usize,
        metric: &mut Metric,
    ) -> Result<(), Error> {
        self.step(1)?;
        self.check(
            Resource::Depth,
            depth as u128,
            self.limits.max_symbol_depth as u128,
        )?;
        match term {
            Template::Value(symbol) => self.symbol_check(symbol, depth, metric)?,
            Template::Variable(slot) => {
                self.measure_reference(
                    binding[*slot]
                        .as_ref()
                        .expect("safe observation variable")
                        .borrow(),
                    depth,
                    metric,
                )?;
            }
            Template::Unary(UnaryOp::Negate, argument) => {
                self.measure(argument, binding, depth, metric)?;
            }
            Template::Unary(_, _) | Template::Binary(_, _, _) | Template::Absolute(_) => {
                self.numeric(term, binding)?;
                self.check(
                    Resource::Nodes,
                    metric.nodes as u128 + 1,
                    self.limits.max_symbol_nodes as u128,
                )?;
                metric.nodes += 1;
            }
            Template::Function(_, _, arguments) | Template::Tuple(arguments) => {
                self.check(
                    Resource::Nodes,
                    metric.nodes as u128 + 1,
                    self.limits.max_symbol_nodes as u128,
                )?;
                metric.nodes += 1;
                if let Template::Function(_, name, _) = term {
                    self.payload(name.as_str().len(), metric)?;
                }
                for argument in arguments {
                    self.measure(argument, binding, depth + 1, metric)?;
                }
            }
        }
        Ok(())
    }

    // Source symbols are depth-capped at compilation. Exact child reservations
    // avoid the upstream clone's separate, geometrically grown work stack.
    fn copy_symbol(&mut self, symbol: &Symbol) -> Result<Symbol, Error> {
        self.step(0)?;
        Ok(match symbol {
            Symbol::Infimum => Symbol::Infimum,
            Symbol::Supremum => Symbol::Supremum,
            Symbol::Number(value) => Symbol::Number(*value),
            Symbol::String(value) => Symbol::String(value.clone()),
            Symbol::Function {
                name,
                arguments,
                sign,
            } => {
                let mut values = self.reserve(arguments.len())?;
                for argument in arguments {
                    values.push(self.copy_symbol(argument)?);
                }
                Symbol::Function {
                    name: name.clone(),
                    arguments: values,
                    sign: *sign,
                }
            }
            Symbol::Tuple(arguments) => {
                let mut values = self.reserve(arguments.len())?;
                for argument in arguments {
                    values.push(self.copy_symbol(argument)?);
                }
                Symbol::Tuple(values)
            }
        })
    }

    // Measurement precedes every clone/allocation in this construction. Compiled
    // template recursion is capped at 64; bound structured values convert iteratively.
    fn construct(
        &mut self,
        term: &Template,
        binding: &[Option<Bound<'_>>],
    ) -> Result<Symbol, Error> {
        self.step(0)?;
        Ok(match term {
            Template::Value(symbol) => self.copy_symbol(symbol)?,
            Template::Variable(slot) => self.copy_reference(
                binding[*slot]
                    .as_ref()
                    .expect("safe observation variable")
                    .borrow(),
            )?,
            Template::Unary(UnaryOp::Negate, argument) => {
                let mut value = self.construct(argument, binding)?;
                match &mut value {
                    Symbol::Number(number) => {
                        *number = crate::scalar_arithmetic::unary(UnaryOp::Negate, *number)
                            .map_err(|cause| self.error(ErrorKind::Evaluation(cause)))?;
                    }
                    Symbol::Function { sign, .. } => {
                        *sign = match sign {
                            Sign::Positive => Sign::Negative,
                            Sign::Negative => Sign::Positive,
                        };
                    }
                    _ => return Err(self.error(ErrorKind::Evaluation(EvaluationError::Undefined))),
                }
                value
            }
            Template::Unary(_, _) | Template::Binary(_, _, _) | Template::Absolute(_) => {
                Symbol::Number(self.numeric(term, binding)?)
            }
            Template::Function(_, _, arguments) | Template::Tuple(arguments) => {
                let mut values = self.reserve(arguments.len())?;
                for argument in arguments {
                    values.push(self.construct(argument, binding)?);
                }
                if let Template::Function(sign, name, _) = term {
                    Symbol::Function {
                        name: name.clone(),
                        arguments: values,
                        sign: *sign,
                    }
                } else {
                    Symbol::Tuple(values)
                }
            }
        })
    }
}

#[derive(Clone, Copy, Default)]
struct Metric {
    nodes: usize,
    bytes: usize,
}
impl Metric {
    fn payload(self) -> u128 {
        (self.nodes as u128) * 16 + self.bytes as u128
    }
}
pub(super) fn scalar_bytes(value: &Value) -> usize {
    match value {
        Value::String(text) | Value::Symbol(text) => text.len(),
        Value::Structured(value) => structured_text_bytes(value),
        _ => 0,
    }
}
pub(super) fn structured_text_bytes(value: &zetesis_core::StructuralValue) -> usize {
    value
        .nodes()
        .iter()
        .map(|node| match node {
            zetesis_core::ValueNode::String(text)
            | zetesis_core::ValueNode::Symbol(text)
            | zetesis_core::ValueNode::Function { name: text, .. } => text.len(),
            _ => 0,
        })
        .sum()
}
fn value_work(value: &Value) -> u128 {
    let nodes = match value {
        Value::Structured(value) => value.nodes().len(),
        _ => 1,
    };
    nodes as u128 + scalar_bytes(value) as u128
}
pub(super) fn scalar(value: &Value) -> Result<Symbol, ErrorKind> {
    Ok(match value {
        Value::Infimum => Symbol::Infimum,
        Value::Supremum => Symbol::Supremum,
        Value::Number(number) => Symbol::Number(*number),
        Value::Structured(value) => {
            crate::structural_value::to_symbol(value).map_err(|error| match error {
                crate::structural_value::BridgeError::Allocation => ErrorKind::Allocation,
                crate::structural_value::BridgeError::InvalidName => ErrorKind::InvalidSymbol,
            })?
        }
        Value::String(text) => Symbol::String(text.clone()),
        Value::Symbol(text) => Symbol::Function {
            name: Name::new(text.clone()).map_err(|_| ErrorKind::InvalidSymbol)?,
            arguments: Vec::new(),
            sign: Sign::Positive,
        },
    })
}
#[derive(Clone, Copy)]
enum Reference<'a> {
    Value(&'a Value),
    Symbol(&'a Symbol),
}
enum Bound<'a> {
    Borrowed(Reference<'a>),
    Owned(Symbol, Metric),
}
impl Bound<'_> {
    fn borrow(&self) -> Reference<'_> {
        match self {
            Self::Borrowed(reference) => *reference,
            Self::Owned(symbol, _) => Reference::Symbol(symbol),
        }
    }
}
fn resolve<'a>(operand: &'a Operand, binding: &'a [Option<Bound<'_>>]) -> Option<Reference<'a>> {
    match operand {
        Operand::Value(value) => Some(Reference::Value(value)),
        Operand::Variable(slot) => binding[*slot].as_ref().map(Bound::borrow),
        Operand::Any | Operand::Function(_, _, _) | Operand::Tuple(_) | Operand::Expression(_) => {
            None
        }
    }
}
fn matches<'a>(
    pattern: &Pattern,
    atom: &'a Atom,
    binding: &mut [Option<Bound<'a>>],
    undo: &mut Vec<usize>,
    bind: bool,
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    work.step(1 + pattern.predicate.name().len() as u128 + atom.predicate().name().len() as u128)?;
    if pattern.predicate != *atom.predicate() {
        return Ok(false);
    }
    for (term, value) in pattern.terms.iter().zip(atom.values()) {
        work.step(1)?;
        if let Some(expected) = resolve(term, binding) {
            if work.compare_reference(expected, Reference::Value(value))? != Ordering::Equal {
                return Ok(false);
            }
        } else if let Operand::Variable(slot) = term {
            debug_assert!(bind, "only positive patterns bind variables");
            binding[*slot] = Some(Bound::Borrowed(Reference::Value(value)));
            undo.push(*slot);
        } else if !patterns::matches_value(term, value, binding, undo, bind, work)? {
            return Ok(false);
        }
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
fn test_pattern(
    pattern: &Pattern,
    atom: &Atom,
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    work.step(1 + pattern.predicate.name().len() as u128 + atom.predicate().name().len() as u128)?;
    if pattern.predicate != *atom.predicate() {
        return Ok(false);
    }
    for (term, value) in pattern.terms.iter().zip(atom.values()) {
        work.step(1)?;
        if !patterns::test_value(term, value, binding, work)? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn condition(
    condition: &Condition,
    atoms: &[&Atom],
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    work.step(1)?;
    match condition {
        Condition::Atom(negation, pattern) => {
            let mut present = false;
            for &atom in atoms {
                if test_pattern(pattern, atom, binding, work)? {
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
                truth = relation_holds(*relation, work.compare_templates(left, right, binding)?);
                if !truth {
                    break;
                }
                left = right;
            }
            Ok(truth != (*negation == DefaultNegation::Not))
        }
        Condition::Boolean(truth) => Ok(*truth),
        Condition::Conditional(query, consequent) => {
            visit(query, atoms, binding, work, &mut |local, work| {
                self::condition(consequent, atoms, local, work)
            })
        }
        Condition::Aggregate(negation, aggregate, guards) => {
            let (value, metric) = scopes::aggregate(aggregate, atoms, binding, work)?;
            let mut truth = true;
            for guard in guards {
                let mut bound_metric = Metric::default();
                work.measure(&guard.bound, binding, 1, &mut bound_metric)?;
                work.construction_check(Metric {
                    nodes: metric.nodes + bound_metric.nodes,
                    bytes: metric.bytes + bound_metric.bytes,
                })?;
                let bound = work.construct(&guard.bound, binding)?;
                work.step(metric.payload() + bound_metric.payload())?;
                if !relation_holds(guard.relation, value.compare(&bound)) {
                    truth = false;
                    break;
                }
            }
            Ok(truth != (*negation == DefaultNegation::Not))
        }
    }
}
fn conditions(
    query: &Query,
    atoms: &[&Atom],
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    for test in &query.conditions {
        if !condition(test, atoms, binding, work)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn insert(
    term: Symbol,
    metric: Metric,
    result: &mut Vec<(Symbol, Metric)>,
    bytes: &mut u128,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    let mut lower = 0;
    let mut upper = result.len();
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        work.step(metric.payload() + result[middle].1.payload() + 1)?;
        match term.cmp(&result[middle].0) {
            Ordering::Equal => return Ok(()),
            Ordering::Less => upper = middle,
            Ordering::Greater => lower = middle + 1,
        }
    }
    work.check(
        Resource::Terms,
        result.len() as u128 + 1,
        work.limits.max_terms as u128,
    )?;
    work.check(
        Resource::OutputBytes,
        *bytes + metric.payload(),
        work.limits.max_output_bytes as u128,
    )?;
    work.step((result.len() - lower) as u128 + 1)?;
    result
        .try_reserve(1)
        .map_err(|_| work.error(ErrorKind::Allocation))?;
    *bytes += metric.payload();
    result.insert(lower, (term, metric));
    Ok(())
}
type Visitor<'a> = dyn FnMut(&[Option<Bound<'_>>], &mut Work<'_>) -> Result<bool, Error> + 'a;

fn complete<'a>(
    query: &Query,
    atoms: &[&'a Atom],
    binding: &mut [Option<Bound<'a>>],
    work: &mut Work<'_>,
    visitor: &mut Visitor<'_>,
) -> Result<bool, Error> {
    work.step(1)?;
    work.check(
        Resource::Bindings,
        u128::from(work.statistics.bindings) + 1,
        u128::from(work.limits.max_bindings),
    )?;
    work.statistics.bindings += 1;
    if conditions(query, atoms, binding, work)? {
        visitor(binding, work)
    } else {
        Ok(true)
    }
}

fn visit<'a>(
    query: &Query,
    atoms: &[&'a Atom],
    outer: &'a [Option<Bound<'a>>],
    work: &mut Work<'_>,
    visitor: &mut Visitor<'_>,
) -> Result<bool, Error> {
    work.step(1 + query.variables as u128 + query.binders.len() as u128)?;
    let mut binding = work.reserve(query.variables)?;
    binding.extend(
        outer
            .iter()
            .map(|value| value.as_ref().map(|value| Bound::Borrowed(value.borrow()))),
    );
    binding.resize_with(query.variables, || None);
    let result = (|| {
        if query.binders.is_empty() {
            return complete(query, atoms, &mut binding, work, visitor);
        }
        let mut cursors = work.reserve(query.binders.len())?;
        cursors.resize(query.binders.len(), 0usize);
        let mut undos: Vec<Vec<usize>> = work.reserve(query.binders.len())?;
        for binder in &query.binders {
            let count = match binder {
                Binder::Atom(pattern) => pattern.terms.iter().map(patterns::slots).sum(),
                Binder::Assign(_, _) => 1,
            };
            undos.push(work.reserve(count)?);
        }
        let mut depth = 0;
        loop {
            work.step(1 + undos[depth].len() as u128)?;
            for slot in undos[depth].drain(..) {
                if let Some(Bound::Owned(_, metric)) = binding[slot].take() {
                    work.local_bytes -= metric.payload();
                }
            }
            let count = match &query.binders[depth] {
                Binder::Atom(_) => atoms.len(),
                Binder::Assign(_, _) => 1,
            };
            if cursors[depth] == count {
                cursors[depth] = 0;
                if depth == 0 {
                    break;
                }
                depth -= 1;
                continue;
            }
            let cursor = cursors[depth];
            cursors[depth] += 1;
            match &query.binders[depth] {
                Binder::Atom(pattern) => {
                    if !matches(
                        pattern,
                        atoms[cursor],
                        &mut binding,
                        &mut undos[depth],
                        true,
                        work,
                    )? {
                        continue;
                    }
                }
                Binder::Assign(slot, expression) => {
                    let (value, metric) = work.own(expression, &binding)?;
                    binding[*slot] = Some(Bound::Owned(value, metric));
                    undos[depth].push(*slot);
                }
            }
            if depth + 1 == query.binders.len() {
                if !complete(query, atoms, &mut binding, work, visitor)? {
                    return Ok(false);
                }
            } else {
                depth += 1;
            }
        }
        Ok(true)
    })();
    // The visitor may retain aggregate keys in its enclosing scope. Release only
    // this query's owned bindings, on success, early termination, and error alike.
    work.local_bytes -= binding
        .iter()
        .filter_map(|bound| match bound {
            Some(Bound::Owned(_, metric)) => Some(metric.payload()),
            _ => None,
        })
        .sum::<u128>();
    result
}

pub(super) fn terms(
    program: &ObservationProgram,
    model: &Model,
    work: &mut Work<'_>,
) -> Result<Vec<Symbol>, Error> {
    work.step(0)?;
    if program.is_empty() {
        return Ok(Vec::new());
    }
    work.step(model.atoms().len() as u128)?;
    let mut atoms = work.reserve(model.atoms().len())?;
    atoms.extend(model.atoms());
    let mut result = Vec::new();
    let mut bytes = 0;
    for directive in &program.directives {
        work.location = directive.origins.first().copied();
        visit(&directive.query, &atoms, &[], work, &mut |binding, work| {
            let mut metric = Metric::default();
            work.measure(&directive.term, binding, 1, &mut metric)?;
            work.construction_check(metric)?;
            let term = work.construct(&directive.term, binding)?;
            insert(term, metric, &mut result, &mut bytes, work)?;
            Ok(true)
        })?;
    }
    work.step(result.len() as u128)?;
    let mut symbols = work.reserve(result.len())?;
    symbols.extend(result.into_iter().map(|(symbol, _)| symbol));
    Ok(symbols)
}
pub(super) fn evaluate(
    program: &ObservationProgram,
    model: &Model,
    limits: Limits,
    construction: ConstructionLimits,
    control: &Control,
) -> Result<Evaluation, Error> {
    let mut work = Work {
        limits,
        construction,
        control,
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
