//! Distinct explicitly admitted roots, independent of canonical DAG population.

use crate::ProgramSite;
use zetesis_core::catalog::{AssignmentError, AssignmentFailure, Error, TermKey, TermSet};

use crate::formula::ceiling;
use crate::formula_support::{Counters, StorageLease, components::Admission};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

pub(crate) struct Domain {
    roots: TermSet,
    lease: StorageLease,
}
impl Domain {
    pub(super) fn new(
        source: &Admission<'_>,
        limits: &FormulaLimits,
        counters: &Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut lease = source.lease();
        lease.observe(size_of::<Self>(), location)?;
        source.storage_observed(&lease, 0, size_of::<Self>(), limits, counters, location)?;
        Ok(Self {
            roots: source.read().term_set(),
            lease,
        })
    }

    pub(super) fn insert(
        &mut self,
        key: &TermKey,
        source: &Admission<'_>,
        maximum: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        if self
            .roots
            .contains_with(key, || counters.work(limits, location))
            .map_err(|error| crate::formula_binding::failure(error, location))?
        {
            return Ok(false);
        }
        ceiling(
            FormulaResource::DomainValues,
            self.roots.len() as u128 + 1,
            maximum as u128,
            location,
        )?;
        let extra = size_of::<Self>() - size_of::<TermSet>();
        let allowance = source.allowance(&self.lease, limits, location)?;
        let previous = self.lease.bytes();
        let result = self
            .roots
            .insert_with(key, allowance.saturating_sub(extra), || {
                counters.work(limits, location)
            })
            .map_err(|error| match error {
                AssignmentFailure::Assignment(AssignmentError::Storage(Error::Storage {
                    required,
                    limit,
                })) => AssignmentFailure::Assignment(AssignmentError::Storage(Error::Storage {
                    required: required + extra as u128,
                    limit: limit + extra,
                })),
                error => error,
            });
        self.lease
            .observe(extra + self.roots.retained_bytes(), location)?;
        let observed = source.storage_observed(
            &self.lease,
            previous,
            size_of::<Self>(),
            limits,
            counters,
            location,
        );
        let inserted = source.storage_result(result, &self.lease, limits, location)?;
        observed?;
        Ok(inserted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula_support::SupportCatalog;
    use themelios_base::{
        source::SourceId,
        span::{ByteOffset, Span},
    };
    use zetesis_core::{Value, ValueLimits, ValueNode};

    fn location() -> ProgramSite {
        ProgramSite::source(themelios_base::span::Location {
            source: SourceId::new(17),
            span: Span::empty(ByteOffset::new(0)),
        })
    }

    #[test]
    fn implicit_subterms_do_not_enlarge_source_domain() {
        let limits = FormulaLimits::default();
        let mut counters = Counters::default();
        let mut catalog = SupportCatalog::default();
        let mut source = catalog
            .component_admission(&limits, &mut counters, location())
            .unwrap();
        let mut domain = Domain::new(&source, &limits, &counters, location()).unwrap();
        let value = Value::from_nodes(
            vec![
                ValueNode::Tuple { arity: 2 },
                ValueNode::Number(1),
                ValueNode::Number(2),
            ],
            ValueLimits::default(),
        )
        .unwrap();
        let key = source
            .import((&value).into(), &limits, &mut counters, location())
            .unwrap();
        assert!(
            domain
                .insert(&key, &source, 1, &limits, &mut counters, location())
                .unwrap()
        );
        assert!(
            !domain
                .insert(&key, &source, 1, &limits, &mut counters, location())
                .unwrap()
        );
        let child = source
            .import(
                (&Value::Number(1)).into(),
                &limits,
                &mut counters,
                location(),
            )
            .unwrap();
        assert!(matches!(
            domain.insert(&child, &source, 1, &limits, &mut counters, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::DomainValues,
                observed: 2,
                limit: 1,
                ..
            })
        ));
    }

    #[test]
    fn scoped_domains_select_roots_independently() {
        let limits = FormulaLimits::default();
        let mut counters = Counters::default();
        let mut catalog = SupportCatalog::default();
        let mut source = catalog
            .component_admission(&limits, &mut counters, location())
            .unwrap();
        let mut original = Domain::new(&source, &limits, &counters, location()).unwrap();
        let mut observation = Domain::new(&source, &limits, &counters, location()).unwrap();
        let left = source
            .import(
                (&Value::Number(1)).into(),
                &limits,
                &mut counters,
                location(),
            )
            .unwrap();
        let right = source
            .import(
                (&Value::Number(2)).into(),
                &limits,
                &mut counters,
                location(),
            )
            .unwrap();
        assert!(
            original
                .insert(&left, &source, 1, &limits, &mut counters, location())
                .unwrap()
        );
        assert!(
            observation
                .insert(&right, &source, 1, &limits, &mut counters, location())
                .unwrap()
        );
        assert!(
            original
                .insert(&right, &source, 1, &limits, &mut counters, location())
                .is_err()
        );
    }
}
