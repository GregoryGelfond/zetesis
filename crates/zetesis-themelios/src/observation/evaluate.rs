//! Model-relative bounded joins. No source carrier or solver is consulted.

use std::cmp::Ordering;

use themelios_base::span::Location;
use themelios_program::symbol::{Name, Sign};
use themelios_program::term::UnaryOp;
use zetesis_core::Atom;

use super::{
    Condition, ConstructionLimits, Control, DefaultNegation, Directive, Error, ErrorKind,
    Evaluation, EvaluationError, Limits, Model, ObservationProgram, Operand, Pattern, Relation,
    Resource, Statistics, Symbol, Template, Value,
};

pub(super) struct Work<'a> {
    pub limits: Limits,
    pub construction: ConstructionLimits,
    pub control: &'a Control,
    pub statistics: Statistics,
    pub location: Option<Location>,
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
    fn numeric(&mut self, term: &Template, binding: &[Option<&Value>]) -> Result<i32, Error> {
        self.step(1)?;
        let result = match term {
            Template::Value(Symbol::Number(value)) => Ok(*value),
            Template::Variable(slot) => match binding[*slot] {
                Some(Value::Number(value)) => Ok(*value),
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
        binding: &[Option<&Value>],
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
        binding: &[Option<&Value>],
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
                let value =
                    binding[*slot].expect("compiled observation variable has a positive binder");
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
                self.payload(scalar_bytes(value), metric)?;
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
    fn construct(&mut self, term: &Template, binding: &[Option<&Value>]) -> Result<Symbol, Error> {
        self.step(0)?;
        Ok(match term {
            Template::Value(symbol) => self.copy_symbol(symbol)?,
            Template::Variable(slot) => scalar(binding[*slot].expect("safe observation variable"))
                .map_err(|kind| self.error(kind))?,
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
fn resolve<'a>(operand: &'a Operand, binding: &[Option<&'a Value>]) -> Option<&'a Value> {
    match operand {
        Operand::Value(value) => Some(value),
        Operand::Variable(slot) => binding[*slot],
        Operand::Any => None,
    }
}
fn matches<'a>(
    pattern: &'a Pattern,
    atom: &'a Atom,
    binding: &mut [Option<&'a Value>],
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
            if work.compare(expected, value)? != Ordering::Equal {
                return Ok(false);
            }
        } else if let Operand::Variable(slot) = term {
            debug_assert!(bind, "only positive patterns bind variables");
            binding[*slot] = Some(value);
            undo.push(*slot);
        }
    }
    Ok(true)
}
fn conditions<'a>(
    directive: &'a Directive,
    atoms: &[&'a Atom],
    binding: &mut [Option<&'a Value>],
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    for condition in &directive.conditions {
        work.step(1)?;
        match condition {
            Condition::Atom(negation, pattern) => {
                let mut present = false;
                for &atom in atoms {
                    if matches(pattern, atom, binding, &mut Vec::new(), false, work)? {
                        present = true;
                        break;
                    }
                }
                if present != (*negation == DefaultNegation::NotNot) {
                    return Ok(false);
                }
            }
            Condition::Compare(negation, first, steps) => {
                let mut left = first;
                let mut truth = true;
                for (relation, right) in steps {
                    let order = work.compare_templates(left, right, binding)?;
                    truth = match relation {
                        Relation::Lt => order.is_lt(),
                        Relation::Le => !order.is_gt(),
                        Relation::Gt => order.is_gt(),
                        Relation::Ge => !order.is_lt(),
                        Relation::Eq => order.is_eq(),
                        Relation::Neq => !order.is_eq(),
                    };
                    if !truth {
                        break;
                    }
                    left = right;
                }
                if truth == (*negation == DefaultNegation::Not) {
                    return Ok(false);
                }
            }
            Condition::Boolean(false) => return Ok(false),
            Condition::Boolean(true) => {}
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
fn emit<'a>(
    directive: &'a Directive,
    atoms: &[&'a Atom],
    binding: &mut [Option<&'a Value>],
    result: &mut Vec<(Symbol, Metric)>,
    bytes: &mut u128,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    work.step(1)?;
    work.check(
        Resource::Bindings,
        u128::from(work.statistics.bindings) + 1,
        u128::from(work.limits.max_bindings),
    )?;
    work.statistics.bindings += 1;
    if conditions(directive, atoms, binding, work)? {
        let mut metric = Metric::default();
        work.measure(&directive.term, binding, 1, &mut metric)?;
        work.construction_check(metric)?;
        let term = work.construct(&directive.term, binding)?;
        insert(term, metric, result, bytes, work)?;
    }
    Ok(())
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
        work.step(1 + directive.variables as u128 + directive.positive.len() as u128)?;
        let mut binding = work.reserve(directive.variables)?;
        binding.resize(directive.variables, None);
        if directive.positive.is_empty() {
            emit(
                directive,
                &atoms,
                &mut binding,
                &mut result,
                &mut bytes,
                work,
            )?;
            continue;
        }
        let mut cursors = work.reserve(directive.positive.len())?;
        cursors.resize(directive.positive.len(), 0usize);
        let mut undos = work.reserve(directive.positive.len())?;
        for pattern in &directive.positive {
            undos.push(work.reserve(pattern.terms.len())?);
        }
        let mut depth = 0;
        loop {
            work.step(1)?;
            work.step(undos[depth].len() as u128)?;
            for slot in undos[depth].drain(..) {
                binding[slot] = None;
            }
            if cursors[depth] == atoms.len() {
                cursors[depth] = 0;
                if depth == 0 {
                    break;
                }
                depth -= 1;
                continue;
            }
            let atom = atoms[cursors[depth]];
            cursors[depth] += 1;
            if !matches(
                &directive.positive[depth],
                atom,
                &mut binding,
                &mut undos[depth],
                true,
                work,
            )? {
                continue;
            }
            if depth + 1 == directive.positive.len() {
                emit(
                    directive,
                    &atoms,
                    &mut binding,
                    &mut result,
                    &mut bytes,
                    work,
                )?;
            } else {
                depth += 1;
            }
        }
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
        location: None,
    };
    let symbols = terms(program, model, &mut work)?;
    Ok(Evaluation {
        symbols,
        statistics: work.statistics,
    })
}
