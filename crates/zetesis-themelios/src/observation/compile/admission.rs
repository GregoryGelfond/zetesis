//! Canonical payload admission; recursive source walks remain depth-limited.
use super::{Compiler, Error, ErrorKind, Sign, Symbol};
use crate::metadata::{Constructor, Predicate, Scalar};
use zetesis_core::{
    ValueNodeRef,
    catalog::{AssignmentFailure, Limits, TermKey},
};

impl Compiler<'_> {
    fn metadata(&self, error: crate::MetadataStorageError) -> Error {
        self.error(ErrorKind::Metadata(error))
    }
    pub(super) fn signature(
        &mut self,
        name: &str,
        arity: usize,
        sign: Sign,
    ) -> Result<Predicate, Error> {
        let predicate =
            zetesis_core::Predicate::with_sign(name, arity, crate::coherence::core_sign(sign))
                .map_err(|_| self.error(ErrorKind::InvalidSymbol))?;
        self.admission
            .predicate((&predicate).into(), 0)
            .map_err(|error| self.metadata(error))
    }
    pub(super) fn shape(
        &mut self,
        name: Option<&str>,
        sign: Sign,
        arity: usize,
    ) -> Result<Constructor, Error> {
        let descriptor = match name {
            Some(name) => ValueNodeRef::Function {
                name,
                sign: crate::coherence::core_sign(sign),
                arity,
            },
            None => ValueNodeRef::Tuple { arity },
        };
        self.admission
            .constructor(descriptor, 0)
            .map_err(|error| self.metadata(error))
    }
    pub(super) fn negate(&mut self, shape: Constructor) -> Result<Constructor, Error> {
        if !matches!(
            self.admission
                .read()
                .map_err(|error| self.metadata(error))?
                .constructor(shape),
            Some(ValueNodeRef::Function { .. })
        ) {
            return Err(self.unsupported(super::Feature::Comparison));
        }
        self.admission
            .negate(shape)
            .map_err(|error| self.metadata(error))
    }
    // Only source ingress is visited here. Children retain scoped coordinates,
    // not cloned source or canonical payload. Temporary coordinates remain under
    // the existing source node/arity bounds, outside the named owner capacity.
    fn symbol_key(
        &mut self,
        value: &Symbol,
        depth: usize,
        resolve: bool,
    ) -> Result<TermKey, Error> {
        self.node(depth)?;
        if resolve
            && let Symbol::Function {
                name,
                arguments,
                sign: Sign::Positive,
            } = value
            && arguments.is_empty()
            && let Some(replacement) = self.constants.get(name.as_str())
        {
            return self.symbol_key(replacement, depth, false);
        }
        let (descriptor, children): (_, &[_]) = match value {
            Symbol::Infimum => (ValueNodeRef::Infimum, &[]),
            Symbol::Supremum => (ValueNodeRef::Supremum, &[]),
            Symbol::Number(value) => (ValueNodeRef::Number(*value), &[]),
            Symbol::String(text) => {
                self.text(text)?;
                (ValueNodeRef::String(text), &[])
            }
            Symbol::Function {
                name,
                arguments,
                sign,
            } => {
                self.text(name.as_str())?;
                self.arity(arguments.len())?;
                (
                    ValueNodeRef::Function {
                        name: name.as_str(),
                        sign: crate::coherence::core_sign(*sign),
                        arity: arguments.len(),
                    },
                    arguments,
                )
            }
            Symbol::Tuple(arguments) => {
                self.arity(arguments.len())?;
                (
                    ValueNodeRef::Tuple {
                        arity: arguments.len(),
                    },
                    arguments,
                )
            }
        };
        let mut assignment = self.admission.catalog().assignment();
        assignment
            .resize_with(children.len(), usize::MAX, || {
                Ok::<_, std::convert::Infallible>(())
            })
            .map_err(|error| self.assignment_error(error))?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(children.len())
            .map_err(|_| self.error(ErrorKind::Allocation))?;
        for (slot, child) in children.iter().enumerate() {
            let key = self.symbol_key(child, depth + 1, resolve)?;
            assignment
                .set_with(slot, &key, || Ok::<_, std::convert::Infallible>(()))
                .map_err(|error| self.assignment_error(error))?;
            slots.push(slot);
        }
        self.admission
            .construct(
                descriptor,
                assignment.as_slice(),
                &slots,
                Limits {
                    max_nodes: self.limits.max_nodes as usize,
                    max_depth: self.limits.max_depth as usize,
                    max_bytes: usize::MAX,
                },
                0,
            )
            .map_err(|error| self.metadata(error))
    }
    fn assignment_error(&self, error: AssignmentFailure<std::convert::Infallible>) -> Error {
        match error {
            AssignmentFailure::Assignment(error) => {
                self.metadata(crate::MetadataStorageError::Assignment(error))
            }
            AssignmentFailure::Stopped(impossible) => match impossible {},
        }
    }
    pub(super) fn symbol(
        &mut self,
        value: &Symbol,
        depth: usize,
        resolve: bool,
    ) -> Result<Scalar, Error> {
        let key = self.symbol_key(value, depth, resolve)?;
        self.admission
            .scalar(&key, 0)
            .map_err(|error| self.metadata(error))
    }
    pub(super) fn revisit_shape(&mut self, shape: Constructor) -> Result<(), Error> {
        let length = match self
            .admission
            .read()
            .map_err(|error| self.metadata(error))?
            .constructor(shape)
            .expect("compiler shape admitted in its authority")
        {
            ValueNodeRef::Function { name, .. } => Some(name.len()),
            _ => None,
        };
        if let Some(length) = length {
            self.text_bytes(length)?;
        }
        Ok(())
    }
    pub(super) fn revisit_scalar(&mut self, scalar: Scalar, depth: usize) -> Result<(), Error> {
        // Existing alternative expansion charges every expanded source symbol
        // again, including text on intern hits. Navigation never copies payload.
        let count = self
            .admission
            .read()
            .map_err(|error| self.metadata(error))?
            .scalar(scalar)
            .expect("admitted scalar")
            .expanded_nodes();
        let mut remaining = [0usize; 64];
        let mut height = 0;
        for rank in 0..count {
            while height > 0 && remaining[height - 1] == 0 {
                height -= 1;
            }
            let node_depth = depth + height;
            if height > 0 {
                remaining[height - 1] -= 1;
            }
            let (text, arity) = {
                let read = self
                    .admission
                    .read()
                    .map_err(|error| self.metadata(error))?;
                let root = read.scalar(scalar).expect("admitted scalar");
                let node = root
                    .subterm_with(rank, || Ok::<_, std::convert::Infallible>(()))
                    .expect("infallible visitor")
                    .expect("rank below expanded count");
                match node.descriptor() {
                    ValueNodeRef::String(text) | ValueNodeRef::Symbol(text) => {
                        (Some(text.len()), 0)
                    }
                    ValueNodeRef::Function { name, arity, .. } => (Some(name.len()), arity),
                    ValueNodeRef::Tuple { arity } => (None, arity),
                    _ => (None, 0),
                }
            };
            self.node(node_depth)?;
            if let Some(length) = text {
                self.text_bytes(length)?;
            }
            self.arity(arity)?;
            if arity > 0 {
                remaining[height] = arity;
                height += 1;
            }
        }
        Ok(())
    }
}
