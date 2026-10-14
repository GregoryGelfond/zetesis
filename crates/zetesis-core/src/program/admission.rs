//! Rule policy and topology over the common canonical template storage.

use crate::catalog::{PredicateRef, TermRef};
use crate::template::catalog::TermData;
use crate::{
    AtomPattern, FilterRef, PatternRef, Template, TemplateCatalogBuilder, TemplateTerm, Term,
};

use super::{
    AdmissionError, AdmissionLimits, AdmissionOrigin, AdmissionResource, FactTables, PredicateId,
    ProgramData, TemplateData, TermId, check_limit, facts,
};

pub(super) fn admit(
    templates: Vec<Template>,
    facts: FactTables,
    limits: AdmissionLimits,
) -> Result<ProgramData, AdmissionError> {
    if let Some(error) = &facts.failed {
        return Err(AdmissionError::Canonical {
            error: error.clone(),
            origin: AdmissionOrigin::Program,
        });
    }
    let count = templates
        .len()
        .checked_add(facts.len())
        .ok_or(AdmissionError::Canonical {
            error: crate::catalog::Error::Overflow,
            origin: AdmissionOrigin::Program,
        })?;
    for block in &facts.blocks {
        if block.before > templates.len() {
            return Err(AdmissionError::Canonical {
                error: crate::catalog::Error::Shape,
                origin: AdmissionOrigin::Fact(block.first),
            });
        }
    }
    check_limit(
        AdmissionResource::Templates,
        count,
        limits.max_templates,
        None,
    )?;
    let builder = TemplateCatalogBuilder::new(limits.max_bytes).map_err(|error| {
        AdmissionError::Canonical {
            error,
            origin: AdmissionOrigin::Program,
        }
    })?;
    let mut admission = Admission {
        builder,
        metadata: size_of::<ProgramData>() as u128,
        template: None,
        fact: None,
        limits,
    };
    admission.publish_metadata()?;
    let mut symbols = Symbols::default();
    let mut admitted = Vec::new();
    admission.reserve(&mut admitted, templates.len())?;
    let mut fact_inputs = facts.blocks.into_iter().peekable();
    let mut prepared = facts::Prepared::default();
    for (index, template) in templates.into_iter().enumerate() {
        while fact_inputs
            .peek()
            .is_some_and(|input| input.before == index)
        {
            facts::append(
                &mut admission,
                &mut prepared,
                fact_inputs.next().expect("peeked fact block"),
                &mut symbols,
            )?;
        }
        admission.fact = None;
        admission.template = Some(index);
        admission.template(&template, index, &mut symbols, &mut admitted)?;
    }
    for input in fact_inputs {
        facts::append(&mut admission, &mut prepared, input, &mut symbols)?;
    }
    admission.template = None;
    admission.fact = None;
    symbols.finish(&admission)?;
    let catalog = admission
        .builder
        .finish()
        .map_err(|error| AdmissionError::Canonical {
            error,
            origin: AdmissionOrigin::Program,
        })?;
    let scratch = prepared.bytes();
    let facts = facts::close(
        &catalog,
        prepared,
        catalog.storage_bytes() + admission.metadata,
        limits,
    )?;
    admission.metadata -= scratch;
    Ok(ProgramData {
        catalog,
        facts,
        templates: admitted,
        domain: symbols.domain,
        predicates: symbols.predicates,
        gate_predicates: symbols.gates,
        metadata_bytes: admission.metadata,
    })
}

#[derive(Default)]
pub(super) struct Symbols {
    pub(super) domain: Vec<TermId>,
    pub(super) predicates: Vec<PredicateId>,
    gates: Vec<PredicateId>,
}
impl Symbols {
    fn finish(&mut self, admission: &Admission) -> Result<(), AdmissionError> {
        self.domain.sort_unstable();
        self.domain.dedup();
        check_limit(
            AdmissionResource::DomainValues,
            self.domain.len(),
            admission.limits.max_domain_values,
            None,
        )?;
        self.predicates.sort_unstable();
        self.predicates.dedup();
        self.gates.sort_unstable();
        self.gates.dedup();
        let read = admission.builder.read();
        self.domain.sort_unstable_by(|left, right| {
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
        self.predicates.sort_unstable_by(compare);
        self.gates.sort_unstable_by(compare);
        Ok(())
    }
}
pub(super) struct Admission {
    pub(super) builder: TemplateCatalogBuilder,
    metadata: u128,
    pub(super) template: Option<usize>,
    pub(super) fact: Option<usize>,
    pub(super) limits: AdmissionLimits,
}
impl Admission {
    pub(super) fn error(&self, error: crate::catalog::Error) -> AdmissionError {
        let origin = if let Some(fact) = self.fact {
            AdmissionOrigin::Fact(fact)
        } else if let Some(template) = self.template {
            AdmissionOrigin::Template(template)
        } else {
            AdmissionOrigin::Program
        };
        AdmissionError::Canonical { error, origin }
    }
    fn template(
        &mut self,
        template: &Template,
        index: usize,
        symbols: &mut Symbols,
        admitted: &mut Vec<TemplateData>,
    ) -> Result<(), AdmissionError> {
        self.validate(template, index)?;
        let row = self
            .builder
            .append(
                std::iter::empty::<TemplateTerm<'_>>(),
                template.patterns().map(PatternRef::from),
                template.filters().iter().map(FilterRef::from),
            )
            .map_err(|error| self.error(error))?;
        let head = template.head().map(|_| 0);
        let start = usize::from(head.is_some());
        let positive = start..start + template.positive().len();
        let gate_true = positive.end..positive.end + template.gate_true().len();
        let gate_false = gate_true.end..gate_true.end + template.gate_false().len();
        self.symbols(row, gate_true.start, symbols)?;
        admitted.push(TemplateData {
            row,
            head,
            positive,
            gate_true,
            gate_false,
            variable_count: template.variable_count(),
        });
        Ok(())
    }
    fn publish_metadata(&mut self) -> Result<(), AdmissionError> {
        self.builder
            .set_external_bytes(self.metadata)
            .map_err(|error| self.error(error))
    }
    pub(super) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), AdmissionError> {
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
    pub(super) fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), AdmissionError> {
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
