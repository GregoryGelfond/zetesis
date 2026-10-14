//! Source compatibility wrapper over the shared neutral producer engine.
use crate::numeric::{self, Classification, Error, Values};
use crate::{Analysis, Argument, Context, Domain, Limits, Statistics, Status, UnknownReason};
use std::{cmp::Ordering, collections::BTreeMap};
use themelios_program::{program::Program, symbol::Symbol};

pub(super) struct Symbols<'p>(pub std::marker::PhantomData<&'p Symbol>);
impl<'p> Values for Symbols<'p> {
    type Key = &'p Symbol;
    type Error = UnknownReason;
    fn classify(
        &self,
        key: &&'p Symbol,
        limits: &Limits,
        budget: &numeric::Budget,
    ) -> Result<Classification, Error<UnknownReason>> {
        let number = match key {
            Symbol::Number(number) => Some(*number),
            _ => None,
        };
        if limits.max_symbol_nodes == 0 || limits.max_symbol_depth == 0 {
            return Ok(Classification {
                number,
                retain: false,
            });
        }
        let mut stack = Vec::new();
        stack.try_reserve(1)?;
        stack.push((*key, 1_usize));
        let (mut nodes, mut bytes) = (0_usize, 0_u128);
        while let Some((symbol, depth)) = stack.pop() {
            budget.work(1)?;
            nodes += 1;
            let payload = match symbol {
                Symbol::String(value) => value.len(),
                Symbol::Function { name, .. } => name.as_str().len(),
                _ => 0,
            };
            bytes += payload as u128;
            if bytes > u128::from(limits.max_symbol_bytes) {
                return Ok(Classification {
                    number,
                    retain: false,
                });
            }
            budget.bytes(u64::try_from(payload).map_err(|_| Error::InvalidInput)?)?;
            let children = symbol.arguments();
            if nodes as u128 + stack.len() as u128 + children.len() as u128
                > limits.max_symbol_nodes as u128
                || (!children.is_empty() && depth >= limits.max_symbol_depth)
            {
                return Ok(Classification {
                    number,
                    retain: false,
                });
            }
            stack.try_reserve(children.len())?;
            stack.extend(children.iter().map(|child| (child, depth + 1)));
        }
        Ok(Classification {
            number,
            retain: true,
        })
    }
    fn compare(
        &self,
        left: &&Symbol,
        right: &&Symbol,
        budget: &numeric::Budget,
    ) -> Result<Ordering, Error<UnknownReason>> {
        budget.work(1)?;
        for root in [*left, *right] {
            let mut pending = Vec::new();
            pending.try_reserve(1)?;
            pending.push(root);
            while let Some(value) = pending.pop() {
                budget.work(1)?;
                let bytes = match value {
                    Symbol::String(value) => value.len(),
                    Symbol::Function { name, .. } => name.as_str().len(),
                    _ => 0,
                };
                budget.bytes(u64::try_from(bytes).map_err(|_| Error::InvalidInput)?)?;
                pending.try_reserve(value.arguments().len())?;
                pending.extend(value.arguments());
            }
        }
        Ok(left.cmp(right))
    }
}

/// Infer conservative argument bounds over this exact immutable program.
/// The source lowering retains its existing conservative unsupported-shape
/// behavior; the neutral engine also serves already admitted producer input.
/// Global logical stops and unresolved source contexts clear every finite map.
/// Local unsupported producers widen only their output argument. Work receipts
/// count this shared mechanism, not the previous source-only transfer schedule.
///
/// This infallible compatibility API retains its existing host-allocation
/// boundary (panic/abort), rather than inventing a logical limit. Final conversion
/// to the public ordered sets also remains outside the engine work meter: its
/// entries and per-symbol comparison size are bounded by the admitted limits.
/// On completion, `statistics().value_entries` counts the materialized entries;
/// `statistics().work` reports engine and lowering work, not this conversion.
/// The neutral [`numeric::analyze`] entry reports typed allocation failures to
/// fallible users and does not reconstruct source symbols or ordered sets.
///
/// # Panics
/// Panics on a reported host allocation failure or an internal source-lowering
/// or result invariant violation. The host allocator may also abort.
#[must_use]
pub fn analyze(program: &Program, limits: Limits) -> Analysis<'_> {
    let mut signatures = BTreeMap::new();
    let result = numeric::analyze(&Symbols(std::marker::PhantomData), limits, |input| {
        crate::compile::Compiler {
            program,
            input,
            signatures: &mut signatures,
            positions: 0,
        }
        .compile()
    });
    let mut output = Analysis {
        program,
        signatures: BTreeMap::new(),
        arguments: Vec::new(),
        status: Status::FixedPoint,
        context: None,
        statistics: Statistics::default(),
    };
    match result {
        Ok(result) => {
            output.statistics = result.statistics();
            output.context = result.context();
            match result.status() {
                numeric::Status::Stopped(stop) => output.status = Status::Stopped(stop),
                numeric::Status::FixedPoint => {
                    output.signatures = signatures
                        .into_iter()
                        .map(|(signature, (_, start))| (signature, start))
                        .collect();
                    output.arguments = result
                        .arguments()
                        .map(|(id, _)| Argument {
                            domain: result.values(id).map_or(Domain::Unknown, |values| {
                                Domain::Finite(values.copied().collect())
                            }),
                            producers: result
                                .producers(id)
                                .expect("registered argument")
                                .iter()
                                .map(|context| match context {
                                    Context::Statement(source) => *source,
                                    Context::Part(_) => {
                                        unreachable!("only a statement produces an argument")
                                    }
                                })
                                .collect(),
                            widening: result.widening(id),
                        })
                        .collect();
                }
            }
        }
        Err(failure) => {
            output.statistics = failure.statistics;
            output.context = failure.context;
            match failure.error {
                Error::Read(reason) => output.status = Status::Unknown(reason),
                Error::Limit(stop) => output.status = Status::Stopped(stop),
                Error::Allocation(error) => panic!("source domain analysis allocation: {error}"),
                Error::InvalidInput => panic!("source lowering submitted an invalid domain plan"),
            }
        }
    }
    output
}
