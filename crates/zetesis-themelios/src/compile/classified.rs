//! One classified source sequence with evidence parallel to its two streams.

use zetesis_core::{
    AdmissionError, AdmissionLimits, AdmissionOrigin, FactSegment, FactTables, Predicate, Program,
    Template, Value,
};

/// A source rule is classified once, before native admission.
pub(crate) enum Rule {
    Template(Template),
    Fact {
        predicate: Predicate,
        values: Vec<Value>,
    },
}

/// Construction-time order, compact across adjacent fact families. Native fact
/// anchors count templates; evidence continues to count original fact rows.
pub(crate) enum Item {
    Template(usize),
    Facts { end: usize },
}

#[derive(Clone, Copy)]
pub(crate) enum Occurrence {
    Template(usize),
    Fact(usize),
}

pub(crate) struct Compiled<P> {
    pub templates: Vec<Template>,
    pub facts: FactTables,
    pub template_origins: Vec<Vec<P>>,
    pub fact_origins: Vec<Vec<P>>,
    pub order: Vec<Item>,
}

impl<P> Default for Compiled<P> {
    fn default() -> Self {
        Self {
            templates: Vec::new(),
            facts: FactTables::default(),
            template_origins: Vec::new(),
            fact_origins: Vec::new(),
            order: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Admitted<P> {
    pub program: Program,
    pub template_origins: Vec<Vec<P>>,
    pub fact_origins: Vec<Vec<P>>,
}

impl<P: Clone> Compiled<P> {
    pub(crate) fn len(&self) -> usize {
        self.templates.len() + self.facts.len()
    }

    pub(crate) fn origins(&self, occurrence: Occurrence) -> &[P] {
        match occurrence {
            Occurrence::Template(index) => &self.template_origins[index],
            Occurrence::Fact(index) => &self.fact_origins[index],
        }
    }

    pub(crate) fn push(&mut self, rule: Rule, origins: Vec<P>) -> Result<(), AdmissionError> {
        match rule {
            Rule::Template(template) => {
                let index = self.templates.len();
                let failure = || AdmissionError::Canonical {
                    error: zetesis_core::catalog::Error::Allocation,
                    origin: AdmissionOrigin::Template(index),
                };
                self.templates.try_reserve(1).map_err(|_| failure())?;
                self.template_origins
                    .try_reserve(1)
                    .map_err(|_| failure())?;
                self.order.try_reserve(1).map_err(|_| failure())?;
                self.templates.push(template);
                self.template_origins.push(origins);
                self.order.push(Item::Template(index));
                Ok(())
            }
            Rule::Fact { predicate, values } => {
                let first = self.facts.len();
                self.facts
                    .push_row(self.templates.len(), predicate, values)
                    .map_err(|error| AdmissionError::Canonical {
                        error,
                        origin: AdmissionOrigin::Fact(first),
                    })?;
                self.record_facts(first, &origins, || Ok(()), |error| error)
            }
        }
    }

    pub(crate) fn product<E>(
        &mut self,
        predicate: Predicate,
        columns: Vec<Vec<FactSegment>>,
        origins: &[P],
        before: impl FnMut() -> Result<(), E>,
        failure: impl Fn(AdmissionError) -> E,
    ) -> Result<(), E> {
        let first = self.facts.len();
        self.facts
            .push_product(self.templates.len(), predicate, columns)
            .map_err(|error| {
                failure(AdmissionError::Canonical {
                    error,
                    origin: AdmissionOrigin::Fact(first),
                })
            })?;
        self.record_facts(first, origins, before, failure)
    }

    fn record_facts<E>(
        &mut self,
        first: usize,
        origins: &[P],
        mut before: impl FnMut() -> Result<(), E>,
        failure: impl Fn(AdmissionError) -> E,
    ) -> Result<(), E> {
        let end = self.facts.len();
        if first == end {
            return Ok(());
        }
        let allocation = || {
            failure(AdmissionError::Canonical {
                error: zetesis_core::catalog::Error::Allocation,
                origin: AdmissionOrigin::Fact(first),
            })
        };
        self.fact_origins
            .try_reserve(end - first)
            .map_err(|_| allocation())?;
        self.order.try_reserve(1).map_err(|_| allocation())?;
        for _ in first..end {
            before()?;
            self.fact_origins.push(origins.to_vec());
        }
        if let Some(Item::Facts { end: previous }) = self.order.last_mut() {
            *previous = end;
        } else {
            self.order.push(Item::Facts { end });
        }
        Ok(())
    }

    pub(crate) fn admit<E>(
        self,
        limits: AdmissionLimits,
        failure: impl FnOnce(AdmissionError, &[P]) -> E,
    ) -> Result<Admitted<P>, E> {
        let Self {
            templates,
            facts,
            template_origins,
            fact_origins,
            order: _,
        } = self;
        let program = Program::new(templates, facts, limits).map_err(|error| {
            let origins = error
                .template_index()
                .and_then(|index| template_origins.get(index))
                .or_else(|| error.fact_index().and_then(|index| fact_origins.get(index)))
                .map_or(&[][..], Vec::as_slice);
            failure(error, origins)
        })?;
        Ok(Admitted {
            program,
            template_origins,
            fact_origins,
        })
    }
}
