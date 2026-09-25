//! Distinct output roots stay canonical until the public Symbol export boundary.
use super::{Error, ErrorKind, Interpreter, Metric, Resource, Symbol};
use zetesis_core::catalog::{TermAssignment, TermKey, TermRead};
pub(super) struct Terms {
    values: TermAssignment,
    order: Vec<usize>,
    bytes: u128,
}
impl Terms {
    pub fn new(read: TermRead<'_>) -> Self {
        Self {
            values: read.assignment(),
            order: Vec::new(),
            bytes: 0,
        }
    }
    pub fn insert(
        &mut self,
        key: &TermKey,
        metric: Metric,
        ctx: &mut Interpreter<'_, '_, '_>,
    ) -> Result<(), Error> {
        let mut lower = 0;
        let mut upper = self.order.len();
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            let other = self
                .values
                .key(self.order[middle])
                .expect("output slot")
                .expect("output root");
            match ctx.compare(key, &other)? {
                std::cmp::Ordering::Equal => return Ok(()),
                std::cmp::Ordering::Less => upper = middle,
                std::cmp::Ordering::Greater => lower = middle + 1,
            }
        }
        ctx.work.check(
            Resource::Terms,
            self.order.len() as u128 + 1,
            ctx.work.limits.max_terms as u128,
        )?;
        ctx.work.check(
            Resource::OutputBytes,
            self.bytes + metric.payload(),
            ctx.work.limits.max_output_bytes as u128,
        )?;
        ctx.work.step((self.order.len() - lower) as u128 + 1)?;
        self.order
            .try_reserve(1)
            .map_err(|_| ctx.work.error(ErrorKind::Allocation))?;
        let slot = self.values.len();
        self.values
            .resize_with(slot + 1, ctx.work.limits.max_term_storage_bytes, || {
                ctx.work.step(1)
            })
            .map_err(|error| super::binding::failure(error, ctx.work))?;
        ctx.set(&mut self.values, slot, key)?;
        self.bytes += metric.payload();
        self.order.insert(lower, slot);
        Ok(())
    }
    pub fn export(self, ctx: &mut Interpreter<'_, '_, '_>) -> Result<Vec<Symbol>, Error> {
        ctx.work.step(self.order.len() as u128)?;
        let mut result = ctx.work.reserve(self.order.len())?;
        for slot in self.order {
            let key = self
                .values
                .key(slot)
                .expect("output slot")
                .expect("output root");
            let metric = ctx.metric(&key)?;
            ctx.work.construction_check(metric)?;
            let value = ctx.terms.read().term(&key).expect("retained output root");
            let symbol = crate::structural_value::term_symbol_with(
                value,
                ctx.work.construction.max_bytes as u128,
                || ctx.work.step(1),
            )
            .map_err(|error| match error {
                crate::structural_value::BridgeFailure::Stopped(error) => error,
                crate::structural_value::BridgeFailure::Bridge(error) => {
                    ctx.work.error(match error {
                        crate::structural_value::BridgeError::Allocation => ErrorKind::Allocation,
                        crate::structural_value::BridgeError::InvalidName => {
                            ErrorKind::InvalidSymbol
                        }
                        crate::structural_value::BridgeError::Storage { required, limit } => {
                            ErrorKind::Limit {
                                resource: Resource::ConstructionBytes,
                                observed: required,
                                limit,
                            }
                        }
                    })
                }
            })?;
            result.push(symbol);
        }
        Ok(result)
    }
}
