use zetesis_core::{Value, catalog::TermRef};
use zetesis_cpu::Cancellation;

use super::lower::Destination;
use super::{
    AggregateBuild, AggregateComparison, AggregateElement, AggregateError, AggregateExtremum,
    AggregateFamilyBuild, AggregateFamilyLimits, AggregateGuard, AggregateLimits, ExtremumBound,
    ValueExtremumElement, extremum, family, lower, value_extremum,
};
use crate::{AdmissionLimits, FormulaParts, FormulaTransaction, FormulaView, TheoryAdmission};

/// Paired formula storage with reusable, completed aggregate-prefix checks.
///
/// Raw construction transfers both buffers and starts unchecked. Aggregate
/// compilation validates node storage and backward edges before conditions are
/// used; atom-universe and root admission remain final admission's job.
/// Read-only access cannot invalidate checked rows. Append transactions own both
/// suffixes and retain no mutable access to earlier nodes or arena cells.
/// Committed scans can be reused. Rollback restores the validation frontier from
/// its paired checkpoint; extracting parts discards this validation evidence.
///
/// Semantic consumers use [`Self::view`]. There is no node-only extraction,
/// truncation or split: an actual append transaction supplies the paired
/// checkpoint for rollback or detachment. No prefix scan or copy is required.
#[derive(Debug, Default)]
pub struct FormulaNodes {
    parts: FormulaParts,
    validated: usize,
}

impl FormulaNodes {
    /// Transfer already paired raw storage without scanning or copying.
    #[must_use]
    pub const fn new(parts: FormulaParts) -> Self {
        Self {
            parts,
            validated: 0,
        }
    }

    /// The single paired owner, including lengths and actual retained capacities.
    #[must_use]
    pub const fn parts(&self) -> &FormulaParts {
        &self.parts
    }

    /// Borrow the storage-independent graph, including any unchecked prefix.
    #[must_use]
    pub fn view(&self) -> FormulaView<'_> {
        self.parts.view()
    }

    /// Begin an exclusive append with an unforgeable, scoped paired checkpoint.
    #[must_use]
    pub fn transaction(&mut self) -> FormulaTransaction<'_> {
        FormulaTransaction::new(&mut self.parts, &mut self.validated)
    }

    /// Transfer this owner and its roots into one fixed theory-admission attempt.
    ///
    /// Preparation is allocation-free and performs no validation or copy. A
    /// completed topology frontier omits only repeated span/back-edge checking;
    /// final dimensions, occurrence recount, atom IDs and roots remain checked.
    /// Raw or partially checked input retains the full raw admission route.
    /// [`TheoryAdmission::work`] reports that same attempt's remaining scans.
    #[must_use]
    pub fn prepare_admission(
        self,
        atoms: usize,
        roots: Vec<usize>,
        limits: AdmissionLimits,
    ) -> TheoryAdmission {
        TheoryAdmission::new(atoms, self.parts, roots, limits, self.validated)
    }

    /// Transfer both buffers without copying and discard the scan frontier.
    #[must_use]
    pub fn into_parts(self) -> FormulaParts {
        self.parts
    }

    /// Compile one scalar aggregate, committing both suffixes only on success.
    ///
    /// # Errors
    /// Preserves the compiler's typed failure and actual work receipt.
    pub fn append_aggregate(
        &mut self,
        elements: &[AggregateElement],
        comparison: AggregateComparison,
        bound: i64,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        let mut transaction = self.transaction();
        let build =
            transaction.append_aggregate(elements, comparison, bound, limits, cancellation)?;
        transaction.commit();
        Ok(build)
    }

    /// Compile all guards in one atomic, shared family and commit on success.
    ///
    /// # Errors
    /// Preserves the compiler's typed failure and cumulative family work receipt.
    pub fn append_aggregate_family(
        &mut self,
        elements: &[AggregateElement],
        guards: &[AggregateGuard],
        limits: AggregateFamilyLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateFamilyBuild, AggregateError> {
        let mut transaction = self.transaction();
        let build = transaction.append_aggregate_family(elements, guards, limits, cancellation)?;
        transaction.commit();
        Ok(build)
    }

    /// Compile one numeric extremum and commit both buffers only on success.
    ///
    /// # Errors
    /// Preserves the compiler's typed failure and actual work receipt.
    pub fn append_extremum(
        &mut self,
        elements: &[AggregateElement],
        extremum: AggregateExtremum,
        comparison: AggregateComparison,
        bound: impl Into<ExtremumBound>,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        let mut transaction = self.transaction();
        let build = transaction.append_extremum(
            elements,
            extremum,
            comparison,
            bound,
            limits,
            cancellation,
        )?;
        transaction.commit();
        Ok(build)
    }

    /// Compile one owned-value extremum, committing both buffers only on success.
    ///
    /// # Errors
    /// Preserves the compiler's typed failure and actual work receipt.
    pub fn append_value_extremum(
        &mut self,
        elements: &[ValueExtremumElement],
        extremum: AggregateExtremum,
        comparison: AggregateComparison,
        bound: &Value,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        let mut transaction = self.transaction();
        let build = transaction.append_value_extremum(
            elements,
            extremum,
            comparison,
            bound,
            limits,
            cancellation,
        )?;
        transaction.commit();
        Ok(build)
    }

    /// Compile one borrowed-value extremum without changing canonical authorities.
    ///
    /// # Errors
    /// Preserves the compiler's typed failure and actual work receipt.
    pub fn append_value_extremum_refs<'a>(
        &mut self,
        elements: impl Iterator<Item = ValueExtremumElement<TermRef<'a>>>,
        extremum: AggregateExtremum,
        comparison: AggregateComparison,
        bound: TermRef<'_>,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        let mut transaction = self.transaction();
        let build = transaction.append_value_extremum_refs(
            elements,
            extremum,
            comparison,
            bound,
            limits,
            cancellation,
        )?;
        transaction.commit();
        Ok(build)
    }
}

impl FormulaTransaction<'_> {
    /// Append the scalar formula of [`super::append_aggregate`], reusing checked
    /// prefix nodes under this owner's exclusive mutation boundary.
    ///
    /// # Errors
    /// Returns the free function's typed failures with actual work accounting.
    pub fn append_aggregate(
        &mut self,
        elements: &[AggregateElement],
        comparison: AggregateComparison,
        bound: i64,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        lower::append(
            self.destination(),
            elements,
            comparison,
            bound,
            limits,
            cancellation,
        )
    }

    /// Append the ordered roots of [`super::append_aggregate_family`], reusing
    /// completed prefix validation across independent families.
    ///
    /// # Errors
    /// Returns the free function's typed failures. Its limits remain cumulative
    /// across this family; completed earlier calls do not spend this call's budget.
    pub fn append_aggregate_family(
        &mut self,
        elements: &[AggregateElement],
        guards: &[AggregateGuard],
        limits: AggregateFamilyLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateFamilyBuild, AggregateError> {
        family::append(self.destination(), elements, guards, limits, cancellation)
    }

    /// Append the numeric extremum of [`super::append_extremum`], reusing
    /// completed prefix validation.
    ///
    /// # Errors
    /// Returns the free function's typed failures with actual work accounting.
    pub fn append_extremum(
        &mut self,
        elements: &[AggregateElement],
        extremum: AggregateExtremum,
        comparison: AggregateComparison,
        bound: impl Into<ExtremumBound>,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        extremum::append(
            self.destination(),
            elements,
            extremum,
            comparison,
            bound.into(),
            limits,
            cancellation,
        )
    }

    /// Append the typed extremum of [`super::append_value_extremum`], reusing
    /// completed prefix validation.
    ///
    /// # Errors
    /// Returns the free function's typed failures with actual work accounting.
    pub fn append_value_extremum(
        &mut self,
        elements: &[ValueExtremumElement],
        extremum: AggregateExtremum,
        comparison: AggregateComparison,
        bound: &Value,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        self.append_value_extremum_refs(
            elements.iter().map(|element| ValueExtremumElement {
                value: (&element.value).into(),
                condition: element.condition,
            }),
            extremum,
            comparison,
            bound.into(),
            limits,
            cancellation,
        )
    }

    /// Append the borrowed typed extremum of [`super::append_value_extremum_refs`].
    /// Canonical terms keep their original authority; only node-prefix validation
    /// is reused, with every typed comparison still charged.
    ///
    /// # Errors
    /// Returns the free function's typed failures with actual work accounting.
    pub fn append_value_extremum_refs<'a>(
        &mut self,
        elements: impl Iterator<Item = ValueExtremumElement<TermRef<'a>>>,
        extremum: AggregateExtremum,
        comparison: AggregateComparison,
        bound: TermRef<'_>,
        limits: AggregateLimits,
        cancellation: &Cancellation,
    ) -> Result<AggregateBuild, AggregateError> {
        value_extremum::append(
            self.destination(),
            elements,
            extremum,
            comparison,
            bound,
            limits,
            cancellation,
        )
    }

    fn destination(&mut self) -> Destination<'_> {
        Destination::retained(self.transaction())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AggregateErrorKind, Node, NodeView, OperandSpan};

    fn prefix() -> FormulaNodes {
        FormulaNodes::new(
            FormulaParts::new(
                vec![
                    Node::atom(0),
                    Node::or_span(OperandSpan {
                        start: 0,
                        length: 3,
                    }),
                ],
                vec![0, 0, 0],
            )
            .unwrap(),
        )
    }

    #[test]
    fn completed_wide_prefix_scan_is_reused() {
        let mut nodes = prefix();
        let limits = AggregateFamilyLimits::default();
        let first = nodes
            .append_aggregate_family(&[], &[], limits, &Cancellation::default())
            .unwrap();
        assert_eq!(nodes.validated, 2);
        let again = nodes
            .append_aggregate_family(&[], &[], limits, &Cancellation::default())
            .unwrap();
        assert!(first.statistics().work > again.statistics().work);
        assert_eq!(again.statistics().work, 1);
        assert_eq!(nodes.view().node(1).unwrap(), NodeView::Or(&[0, 0, 0]));
    }

    #[test]
    fn failed_family_restores_the_paired_validation_checkpoint() {
        let mut nodes = prefix();
        let limits = AggregateFamilyLimits {
            aggregate: AggregateLimits {
                max_nodes: 2,
                ..AggregateLimits::default()
            },
            ..AggregateFamilyLimits::default()
        };
        let error = nodes
            .append_aggregate_family(
                &[AggregateElement {
                    weight: 1,
                    condition: 1,
                }],
                &[AggregateGuard {
                    comparison: AggregateComparison::Ge,
                    bound: 1,
                }],
                limits,
                &Cancellation::default(),
            )
            .unwrap_err();
        assert_eq!(error.kind, AggregateErrorKind::NodeLimit);
        assert_eq!(nodes.validated, 0);
        assert_eq!(nodes.parts().nodes().len(), 2);
        assert_eq!(nodes.parts().operands(), &[0, 0, 0]);
        assert_eq!(nodes.parts().occurrences(), 3);
    }

    #[test]
    fn family_suffix_can_be_detached_for_remapping() {
        let mut nodes = prefix();
        let mut transaction = nodes.transaction();
        let build = transaction
            .append_aggregate_family(
                &[AggregateElement {
                    weight: 1,
                    condition: 1,
                }],
                &[AggregateGuard {
                    comparison: AggregateComparison::Eq,
                    bound: 1,
                }],
                AggregateFamilyLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        let suffix = transaction.detach().unwrap();
        assert_eq!(suffix.first(), 2);
        assert_eq!(suffix.view().len(), build.appended_nodes());
        for &root in build.roots() {
            assert!(root < suffix.first() + suffix.view().len());
        }
        assert_eq!(nodes.view().len(), 2);
        assert_eq!(nodes.parts().operands(), &[0, 0, 0]);
        assert_eq!(nodes.validated, 0);
    }
}
