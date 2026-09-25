//! Observation construction over immutable input vocabularies and local term IDs.
//! Read borrows end before append; captured input subtrees are registered only
//! when computation needs them. Public Symbol construction belongs to export.

use super::{Binding, Error, ErrorKind, EvaluationError, Metric, Resource, Template, Work};
use std::cmp::Ordering;
use themelios_program::term::UnaryOp;
use zetesis_core::{
    ValueNodeRef,
    catalog::{
        AssignmentSlice, DeclaredConstructor, DerivedFailure, DerivedTerms, TermAssignment,
        TermKey, TermRef,
    },
};

pub(super) struct Interpreter<'input, 'work, 'cancel> {
    pub metadata: crate::metadata::Read<'input>,
    pub terms: DerivedTerms<'input>,
    pub work: &'work mut Work<'cancel>,
    pub keys: super::anonymous::Keys<'input>,
}
impl<'input> Interpreter<'input, '_, '_> {
    pub fn observe(&mut self) {
        self.work.statistics.term_storage_bytes = self.terms.storage_bytes() + self.keys.bytes();
        self.work.statistics.peak_term_storage_bytes = self
            .work
            .statistics
            .peak_term_storage_bytes
            .max(self.work.statistics.term_storage_bytes)
            .max(self.keys.peak());
    }
    fn prepare(&mut self) -> Result<(), Error> {
        let external = self.keys.bytes();
        self.work.check(
            Resource::TermStorageBytes,
            external + self.terms.storage_bytes(),
            self.work.limits.max_term_storage_bytes as u128,
        )?;
        let remaining = usize::try_from(self.work.limits.max_term_storage_bytes as u128 - external)
            .expect("checked allowance");
        self.terms.restart_storage_peak();
        self.terms
            .ceiling(remaining)
            .map_err(|error| self.work.error(ErrorKind::TermStorage(error)))
    }
    fn finish<T>(&mut self, result: Result<T, DerivedFailure<Error>>) -> Result<T, Error> {
        self.work.statistics.peak_term_storage_bytes = self
            .work
            .statistics
            .peak_term_storage_bytes
            .max(self.terms.storage_peak_bytes() + self.keys.bytes());
        self.observe();
        result.map_err(|error| match error {
            DerivedFailure::Storage(zetesis_core::catalog::Error::Storage { required, .. }) => {
                self.work.error(ErrorKind::Limit {
                    resource: Resource::TermStorageBytes,
                    observed: required + self.keys.bytes(),
                    limit: self.work.limits.max_term_storage_bytes as u128,
                })
            }
            DerivedFailure::Storage(error) => self.work.error(ErrorKind::TermStorage(error)),
            DerivedFailure::Read(error) => self.work.error(ErrorKind::TermAssignment(error.into())),
            DerivedFailure::Assignment(error) => self.work.error(ErrorKind::TermAssignment(error)),
            DerivedFailure::Stopped(mut error) => {
                error.statistics = self.work.statistics;
                error
            }
        })
    }
    pub fn value<'a>(&'a self, key: &TermKey) -> TermRef<'a> {
        self.terms
            .read()
            .term(key)
            .expect("interpreter key belongs to its retained arena")
    }
    pub fn input(&mut self, value: TermRef<'input>) -> Result<TermKey, Error> {
        self.prepare()?;
        let result = self.terms.borrow_with(value, || self.work.step(1));
        self.finish(result)
    }
    pub fn child(&mut self, key: &TermKey, index: usize) -> Result<Option<TermKey>, Error> {
        self.prepare()?;
        let result = self.terms.child_with(key, index, || self.work.step(1));
        self.finish(result)
    }
    pub fn scalar(&mut self, value: ValueNodeRef<'_>) -> Result<TermKey, Error> {
        self.prepare()?;
        let logical = self.logical_limits();
        let result = self.terms.scalar_with(value, logical, || self.work.step(1));
        self.finish(result)
    }
    pub fn number(&mut self, value: i32) -> Result<TermKey, Error> {
        self.scalar(ValueNodeRef::Number(value))
    }
    fn logical_limits(&self) -> zetesis_core::catalog::Limits {
        zetesis_core::catalog::Limits {
            max_nodes: self.work.limits.max_symbol_nodes,
            max_depth: self.work.limits.max_symbol_depth,
            // Observation UTF-8 payload and temporary construction bounds are
            // checked separately; core canonical bytes are a different measure.
            max_bytes: usize::MAX,
        }
    }
    pub fn build(
        &mut self,
        shape: &DeclaredConstructor,
        assignment: AssignmentSlice<'_>,
        slots: &[usize],
    ) -> Result<TermKey, Error> {
        self.prepare()?;
        let logical = self.logical_limits();
        let result = self
            .terms
            .construct_with(shape, assignment, slots, logical, || self.work.step(1));
        self.finish(result)
    }
    pub fn frame(&mut self, count: usize) -> Result<TermAssignment, Error> {
        let mut frame = self.terms.read().assignment();
        frame
            .resize_with(count, self.work.limits.max_term_storage_bytes, || {
                self.work.step(1)
            })
            .map_err(|error| super::binding::failure(error, self.work))?;
        Ok(frame)
    }
    pub fn set(
        &mut self,
        frame: &mut TermAssignment,
        slot: usize,
        key: &TermKey,
    ) -> Result<(), Error> {
        frame
            .set_with(slot, key, || self.work.step(1))
            .map_err(|error| super::binding::failure(error, self.work))
    }
    fn binding_key(&mut self, binding: &Binding<'input>, slot: usize) -> Result<TermKey, Error> {
        if let Some(value) = binding.input(slot) {
            self.input(value)
        } else {
            binding
                .key(slot)
                .map_err(|error| self.work.error(ErrorKind::TermAssignment(error)))
        }
    }
    pub fn compare(&mut self, left: &TermKey, right: &TermKey) -> Result<Ordering, Error> {
        let read = self.terms.read();
        let left = read.term(left).expect("retained local key");
        let right = read.term(right).expect("retained local key");
        left.compare_terms_with(right, || self.work.step(1))
    }
    pub fn measure_key(
        &mut self,
        key: &TermKey,
        depth: usize,
        metric: &mut Metric,
    ) -> Result<(), Error> {
        self.work.measure_reference(
            self.terms.read().term(key).expect("retained local key"),
            depth,
            metric,
        )
    }
    pub fn metric(&mut self, key: &TermKey) -> Result<Metric, Error> {
        let mut metric = Metric::default();
        self.work.measure_reference(
            self.terms.read().term(key).expect("local key"),
            1,
            &mut metric,
        )?;
        Ok(metric)
    }
    pub fn numeric(&mut self, term: &Template, binding: &Binding<'input>) -> Result<i32, Error> {
        self.work.step(1)?;
        let result = match term {
            Template::Constant(scalar) => match self
                .metadata
                .scalar(*scalar)
                .expect("compiled scalar")
                .descriptor()
            {
                ValueNodeRef::Number(value) => Ok(value),
                _ => Err(EvaluationError::Undefined),
            },
            Template::Variable(slot) => match binding
                .value(*slot, self.terms.read(), self.work)?
                .map(TermRef::descriptor)
            {
                Some(ValueNodeRef::Number(value)) => Ok(value),
                _ => Err(EvaluationError::Undefined),
            },
            Template::Unary(operator, argument) => {
                crate::scalar_arithmetic::unary(*operator, self.numeric(argument, binding)?)
            }
            Template::Binary(operator, left, right) => {
                let left = self.numeric(left, binding)?;
                let right = self.numeric(right, binding)?;
                crate::scalar_arithmetic::binary(*operator, left, right)
            }
            Template::Absolute(argument) => {
                crate::scalar_arithmetic::absolute(self.numeric(argument, binding)?)
            }
            _ => Err(EvaluationError::Undefined),
        };
        result.map_err(|error| self.work.error(ErrorKind::Evaluation(error)))
    }
    pub fn measure(
        &mut self,
        term: &Template,
        binding: &Binding<'input>,
        depth: usize,
        metric: &mut Metric,
    ) -> Result<(), Error> {
        self.work.step(1)?;
        self.work.check(
            Resource::Depth,
            depth as u128,
            self.work.limits.max_symbol_depth as u128,
        )?;
        match term {
            Template::Constant(scalar) => self.work.measure_reference(
                self.metadata.scalar(*scalar).expect("compiled scalar"),
                depth,
                metric,
            )?,
            Template::Variable(slot) => {
                let value = binding
                    .value(*slot, self.terms.read(), self.work)?
                    .expect("safe scalar variable");
                self.work.measure_reference(value, depth, metric)?;
            }
            Template::Unary(UnaryOp::Negate, argument) => {
                self.measure(argument, binding, depth, metric)?;
            }
            Template::Unary(..) | Template::Binary(..) | Template::Absolute(_) => {
                self.numeric(term, binding)?;
                self.work.node(metric)?;
            }
            Template::Pool(_) | Template::Interval(..) => {
                unreachable!("finite alternatives use expansion")
            }
            Template::Construct(shape, arguments) => {
                self.work.node(metric)?;
                self.work.payload(
                    self.metadata
                        .constructor(*shape)
                        .expect("compiled shape")
                        .text_bytes(),
                    metric,
                )?;
                for argument in arguments {
                    self.measure(argument, binding, depth + 1, metric)?;
                }
            }
        }
        Ok(())
    }
    pub fn own(
        &mut self,
        term: &Template,
        binding: &Binding<'input>,
    ) -> Result<(TermKey, Metric), Error> {
        let mut metric = Metric::default();
        self.measure(term, binding, 1, &mut metric)?;
        self.work.construction_check(metric)?;
        self.work.check(
            Resource::LocalBytes,
            self.work.local_bytes + metric.payload(),
            self.work.limits.max_local_bytes as u128,
        )?;
        self.construct(term, binding).map(|key| (key, metric))
    }
    pub fn construct(
        &mut self,
        term: &Template,
        binding: &Binding<'input>,
    ) -> Result<TermKey, Error> {
        self.work.step(0)?;
        match term {
            Template::Constant(scalar) => {
                self.input(self.metadata.scalar(*scalar).expect("compiled scalar"))
            }
            Template::Variable(slot) => self.binding_key(binding, *slot),
            Template::Unary(UnaryOp::Negate, argument) => {
                let key = self.construct(argument, binding)?;
                self.negate(&key)
            }
            Template::Unary(..) | Template::Binary(..) | Template::Absolute(_) => {
                let number = self.numeric(term, binding)?;
                self.number(number)
            }
            Template::Pool(_) | Template::Interval(..) => {
                unreachable!("finite alternatives use expansion")
            }
            Template::Construct(shape, arguments) => {
                let shape = self
                    .metadata
                    .constructor_declaration(*shape)
                    .expect("compiled shape");
                let mut frame = self.frame(arguments.len())?;
                let mut slots = self.work.reserve(arguments.len())?;
                for (index, argument) in arguments.iter().enumerate() {
                    let key = self.construct(argument, binding)?;
                    self.set(&mut frame, index, &key)?;
                    slots.push(index);
                }
                self.build(&shape, frame.as_slice(), &slots)
            }
        }
    }
    pub fn negate(&mut self, key: &TermKey) -> Result<TermKey, Error> {
        let descriptor = self.value(key).descriptor();
        let (sign, arity) = match descriptor {
            ValueNodeRef::Number(value) => {
                let number = crate::scalar_arithmetic::unary(UnaryOp::Negate, value)
                    .map_err(|error| self.work.error(ErrorKind::Evaluation(error)))?;
                return self.number(number);
            }
            ValueNodeRef::Symbol(_) => (zetesis_core::Sign::Negative, 0),
            ValueNodeRef::Function { sign, arity, .. } => (
                match sign {
                    zetesis_core::Sign::Positive => zetesis_core::Sign::Negative,
                    zetesis_core::Sign::Negative => zetesis_core::Sign::Positive,
                },
                arity,
            ),
            _ => {
                return Err(self
                    .work
                    .error(ErrorKind::Evaluation(EvaluationError::Undefined)));
            }
        };
        let shape = self
            .terms
            .read()
            .constructor_of(key)
            .expect("local constructor")
            .expect("function shape")
            .with_sign(sign)
            .expect("function sign");
        let mut frame = self.frame(arity)?;
        let mut slots = self.work.reserve(arity)?;
        for index in 0..arity {
            let child = self.child(key, index)?.expect("admitted child");
            self.set(&mut frame, index, &child)?;
            slots.push(index);
        }
        self.build(&shape, frame.as_slice(), &slots)
    }
    pub fn compare_templates(
        &mut self,
        left: &Template,
        right: &Template,
        binding: &Binding<'input>,
    ) -> Result<Ordering, Error> {
        let mut metric = Metric::default();
        self.measure(left, binding, 1, &mut metric)?;
        let mut right_metric = Metric::default();
        self.measure(right, binding, 1, &mut right_metric)?;
        self.work.construction_check(Metric {
            nodes: metric.nodes + right_metric.nodes,
            bytes: metric.bytes + right_metric.bytes,
        })?;
        let left = self.construct(left, binding)?;
        let right = self.construct(right, binding)?;
        self.compare(&left, &right)
    }
}
impl Work<'_> {
    pub(super) fn node(&mut self, metric: &mut Metric) -> Result<(), Error> {
        self.check(
            Resource::Nodes,
            metric.nodes as u128 + 1,
            self.limits.max_symbol_nodes as u128,
        )?;
        metric.nodes += 1;
        Ok(())
    }
    pub(super) fn payload(&mut self, bytes: usize, metric: &mut Metric) -> Result<(), Error> {
        self.step(bytes as u128)?;
        let observed = metric.bytes as u128 + bytes as u128;
        self.check(
            Resource::Bytes,
            observed,
            self.limits.max_symbol_bytes as u128,
        )?;
        metric.bytes = usize::try_from(observed).expect("checked usize allowance");
        Ok(())
    }
    pub(super) fn measure_reference(
        &mut self,
        value: TermRef<'_>,
        depth: usize,
        metric: &mut Metric,
    ) -> Result<(), Error> {
        self.check(
            Resource::Depth,
            depth as u128 + value.depth() as u128 - 1,
            self.limits.max_symbol_depth as u128,
        )?;
        let nodes = metric.nodes as u128 + value.expanded_nodes() as u128;
        self.check(Resource::Nodes, nodes, self.limits.max_symbol_nodes as u128)?;
        metric.nodes = usize::try_from(nodes).expect("checked usize allowance");
        let mut cursor = value.nodes();
        while let Some(node) = cursor.next_with(|| self.step(1))? {
            self.payload(node.text_bytes(), metric)?;
        }
        Ok(())
    }
    pub(super) fn construction_check(&self, metric: Metric) -> Result<(), Error> {
        self.check(
            Resource::ConstructionBytes,
            metric.construction_bytes(),
            self.construction.max_bytes as u128,
        )
    }
}
