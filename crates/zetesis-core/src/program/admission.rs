//! Rule policy and topology over the common canonical template storage.

use crate::catalog::{PredicateRef, TermRef};
use crate::template::catalog::TermData;
use crate::{
    AtomPattern, FilterRef, PatternRef, Template, TemplateCatalogBuilder, TemplateTerm, Term,
};

use super::{
    AdmissionError, AdmissionLimits, AdmissionResource, PredicateId, ProgramData, TemplateData,
    TermId, check_limit,
};

pub(super) fn admit(
    templates: Vec<Template>,
    limits: AdmissionLimits,
) -> Result<ProgramData, AdmissionError> {
    check_limit(
        AdmissionResource::Templates,
        templates.len(),
        limits.max_templates,
        None,
    )?;
    let builder = TemplateCatalogBuilder::new(limits.max_bytes).map_err(|error| {
        AdmissionError::Canonical {
            error,
            template: None,
        }
    })?;
    let mut admission = Admission {
        builder,
        metadata: size_of::<ProgramData>() as u128,
        template: None,
        limits,
    };
    admission.publish_metadata()?;
    let mut symbols = Symbols::default();
    let mut admitted = Vec::new();
    admission.reserve(&mut admitted, templates.len())?;
    for (index, template) in templates.into_iter().enumerate() {
        admission.template = Some(index);
        admission.validate(&template, index)?;
        let row = admission
            .builder
            .append(
                std::iter::empty::<TemplateTerm<'_>>(),
                template.patterns().map(PatternRef::from),
                template.filters().iter().map(FilterRef::from),
            )
            .map_err(|error| admission.error(error))?;
        let head = template.head().map(|_| 0);
        let start = usize::from(head.is_some());
        let positive = start..start + template.positive().len();
        let gate_true = positive.end..positive.end + template.gate_true().len();
        let gate_false = gate_true.end..gate_true.end + template.gate_false().len();
        admission.symbols(row, gate_true.start, &mut symbols)?;
        admitted.push(TemplateData {
            row,
            head,
            positive,
            gate_true,
            gate_false,
            variable_count: template.variable_count(),
        });
    }
    admission.template = None;
    symbols.domain.sort_unstable();
    symbols.domain.dedup();
    check_limit(
        AdmissionResource::DomainValues,
        symbols.domain.len(),
        limits.max_domain_values,
        None,
    )?;
    symbols.predicates.sort_unstable();
    symbols.predicates.dedup();
    symbols.gates.sort_unstable();
    symbols.gates.dedup();
    let read = admission.builder.read();
    symbols.domain.sort_unstable_by(|left, right| {
        TermRef::new(read, *left)
            .expect("explicit root was imported by the component builder")
            .cmp(
                &TermRef::new(read, *right)
                    .expect("explicit root was imported by the component builder"),
            )
    });
    let compare = |left: &PredicateId, right: &PredicateId| {
        PredicateRef::new(read, *left)
            .expect("pattern signature was imported by the component builder")
            .cmp(
                &PredicateRef::new(read, *right)
                    .expect("pattern signature was imported by the component builder"),
            )
    };
    symbols.predicates.sort_unstable_by(compare);
    symbols.gates.sort_unstable_by(compare);
    let catalog = admission
        .builder
        .finish()
        .map_err(|error| AdmissionError::Canonical {
            error,
            template: None,
        })?;
    Ok(ProgramData {
        catalog,
        templates: admitted,
        domain: symbols.domain,
        predicates: symbols.predicates,
        gate_predicates: symbols.gates,
        metadata_bytes: admission.metadata,
    })
}

#[derive(Default)]
struct Symbols {
    domain: Vec<TermId>,
    predicates: Vec<PredicateId>,
    gates: Vec<PredicateId>,
}
struct Admission {
    builder: TemplateCatalogBuilder,
    metadata: u128,
    template: Option<usize>,
    limits: AdmissionLimits,
}
impl Admission {
    fn error(&self, error: crate::catalog::Error) -> AdmissionError {
        AdmissionError::Canonical {
            error,
            template: self.template,
        }
    }
    fn publish_metadata(&mut self) -> Result<(), AdmissionError> {
        self.builder
            .set_external_bytes(self.metadata)
            .map_err(|error| self.error(error))
    }
    fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), AdmissionError> {
        let required = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| self.error(crate::catalog::Error::Overflow))?;
        if required <= values.capacity() {
            return Ok(());
        }
        let target = required
            .max(
                values
                    .capacity()
                    .checked_mul(2)
                    .ok_or_else(|| self.error(crate::catalog::Error::Overflow))?,
            )
            .max(4);
        let old = values.capacity() as u128 * size_of::<T>() as u128;
        self.builder
            .set_external_bytes(self.metadata + target as u128 * size_of::<T>() as u128)
            .map_err(|error| self.error(error))?;
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| self.error(crate::catalog::Error::Allocation))?;
        let actual = values.capacity() as u128 * size_of::<T>() as u128;
        self.builder
            .set_external_bytes(self.metadata + actual)
            .map_err(|error| self.error(error))?;
        self.metadata += actual - old;
        self.publish_metadata()
    }
    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), AdmissionError> {
        self.reserve(values, 1)?;
        values.push(value);
        Ok(())
    }
    fn validate(&mut self, template: &Template, index: usize) -> Result<(), AdmissionError> {
        check_limit(
            AdmissionResource::PositiveBody,
            template.positive().len(),
            self.limits.max_positive_body,
            Some(index),
        )?;
        check_limit(
            AdmissionResource::Variables,
            template.variable_count(),
            self.limits.max_variables_per_template,
            Some(index),
        )?;
        let mut variables = Vec::new();
        self.reserve(&mut variables, template.variable_count())?;
        for term in template.all_terms() {
            if let Term::Variable(variable) = term
                && let Err(at) = variables.binary_search(variable)
            {
                variables.insert(at, *variable);
            }
        }
        for (expected, variable) in variables.iter().copied().enumerate() {
            if expected != variable {
                return Err(AdmissionError::NonDenseVariable {
                    template: index,
                    expected,
                    actual: variable,
                });
            }
            if !template
                .positive()
                .iter()
                .flat_map(AtomPattern::terms)
                .any(|term| matches!(term, Term::Variable(bound) if *bound == variable))
            {
                return Err(AdmissionError::UnsafeVariable {
                    template: index,
                    variable,
                });
            }
        }
        self.metadata -= variables.capacity() as u128 * size_of::<usize>() as u128;
        drop(variables);
        self.publish_metadata()?;
        for pattern in template.patterns() {
            check_limit(
                AdmissionResource::PredicateArity,
                pattern.predicate().arity(),
                self.limits.max_predicate_arity,
                Some(index),
            )?;
        }
        Ok(())
    }
    fn symbols(
        &mut self,
        row: usize,
        first_gate: usize,
        symbols: &mut Symbols,
    ) -> Result<(), AdmissionError> {
        let count = self
            .builder
            .data(row)
            .expect("just appended complete row")
            .patterns
            .len();
        for at in 0..count {
            let pattern = &self
                .builder
                .data(row)
                .expect("just appended complete row")
                .patterns[at];
            let predicate = pattern.predicate;
            let terms = pattern.terms.len();
            self.push(&mut symbols.predicates, predicate)?;
            if at >= first_gate {
                self.push(&mut symbols.gates, predicate)?;
            }
            for column in 0..terms {
                let term = self
                    .builder
                    .data(row)
                    .expect("just appended complete row")
                    .patterns[at]
                    .terms[column];
                if let TermData::Constant(id) = term {
                    self.push(&mut symbols.domain, id)?;
                }
            }
        }
        let count = self
            .builder
            .data(row)
            .expect("just appended complete row")
            .filters
            .len();
        for at in 0..count {
            let filter = self
                .builder
                .data(row)
                .expect("just appended complete row")
                .filters[at];
            for term in [filter.left, filter.right] {
                if let TermData::Constant(id) = term {
                    self.push(&mut symbols.domain, id)?;
                }
            }
        }
        Ok(())
    }
}
