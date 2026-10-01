//! Source tests share a real canonical owner and execution workspace.
use super::{CompletedCatalog, Computation, Counters, Support, SupportCatalog};
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::{ExpansionLimits, FormulaLimits};
use themelios_base::span::Location;
use zetesis_core::Value;

#[derive(Default)]
pub(crate) struct Fixture {
    owner: SupportCatalog,
    counters: Counters,
}
impl Fixture {
    pub(crate) fn from_atoms(
        atoms: impl IntoIterator<Item = zetesis_core::Atom>,
        location: Location,
    ) -> Self {
        let mut fixture = Self::default();
        for atom in atoms {
            fixture.owner = fixture
                .owner
                .insert(
                    &atom,
                    &FormulaLimits::default(),
                    &mut fixture.counters,
                    location,
                )
                .unwrap();
        }
        fixture
    }

    /// Complete the empty source through the production fixed-point builder.
    /// Earlier identity-only discoveries remain canonical without becoming facts.
    pub(crate) fn finish(self, location: Location) -> (CompletedCatalog, Counters) {
        let Self {
            mut owner,
            mut counters,
        } = self;
        let mut budget =
            crate::expansion::Budget::new(crate::ExpansionLimits::default(), usize::MAX);
        let (prepared, _) = prepare_into("", &mut owner, &mut counters, &mut budget);
        let completed = super::build(
            owner,
            &prepared,
            None,
            &FormulaLimits::default(),
            &mut budget,
            &mut counters,
            location,
        )
        .unwrap();
        (completed, counters)
    }

    /// Admit source metadata before any query lends the immutable prefix.
    pub(crate) fn admit<T>(
        &mut self,
        location: Location,
        action: impl FnOnce(&mut super::components::Admission<'_>, &mut Counters) -> T,
    ) -> T {
        let limits = FormulaLimits::default();
        let mut source = self
            .owner
            .component_admission(&limits, &mut self.counters, location)
            .unwrap();
        let result = action(&mut source, &mut self.counters);
        source
            .finish(&limits, &mut self.counters, location)
            .unwrap();
        result
    }

    pub(crate) fn scalar(
        &mut self,
        value: &Value,
        location: Location,
    ) -> super::components::Scalar {
        self.admit(location, |source, counters| {
            source
                .scalar(value.into(), &FormulaLimits::default(), counters, location)
                .unwrap()
        })
    }

    pub(crate) fn constructor(
        &mut self,
        descriptor: zetesis_core::ValueNodeRef<'_>,
        location: Location,
    ) -> super::components::Constructor {
        self.admit(location, |source, counters| {
            source
                .constructor(descriptor, &FormulaLimits::default(), counters, location)
                .unwrap()
        })
    }

    /// Admit a test pattern through the same authority as its eventual rows.
    pub(crate) fn pattern(
        &mut self,
        pattern: &zetesis_core::AtomPattern,
        location: Location,
    ) -> super::components::Pattern {
        admit_pattern(&mut self.owner, pattern, &mut self.counters, location)
    }

    pub(crate) fn with<T>(
        &mut self,
        location: Location,
        run: impl FnOnce(&Support<'_>, &mut Computation<'_, '_>, &mut Counters) -> T,
    ) -> T {
        let limits = FormulaLimits::default();
        let (relations, mut append) = self
            .owner
            .split(&limits, &mut self.counters, location)
            .unwrap();
        let support = Support::indexed(&relations, &limits, &self.counters, location).unwrap();
        let mut computation = Computation::new(&mut append, &support);
        run(&support, &mut computation, &mut self.counters)
    }
}

pub(crate) fn admit_pattern(
    catalog: &mut SupportCatalog,
    pattern: &zetesis_core::AtomPattern,
    counters: &mut Counters,
    location: Location,
) -> super::components::Pattern {
    let limits = FormulaLimits::default();
    let mut source = catalog
        .component_admission(&limits, counters, location)
        .unwrap();
    let predicate = source
        .predicate(pattern.predicate().into(), &limits, counters, location)
        .unwrap();
    let arguments: Vec<_> = pattern
        .terms()
        .iter()
        .map(|term| match term {
            zetesis_core::Term::Variable(slot) => super::components::Term::Variable(*slot),
            zetesis_core::Term::Constant(value) => super::components::Term::Constant(
                source
                    .scalar(value.into(), &limits, counters, location)
                    .unwrap(),
            ),
        })
        .collect();
    let pattern = source
        .pattern(predicate, &arguments, &limits, counters, location)
        .unwrap();
    source.finish(&limits, counters, location).unwrap();
    pattern
}

pub(crate) fn binding(
    values: &[Option<Value>],
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    location: Location,
) -> Binding<'static> {
    let limits = FormulaLimits::default();
    let mut binding = Binding::new(computation, &limits, counters, location).unwrap();
    binding
        .extend_scope(values.len(), computation, &limits, counters, location)
        .unwrap();
    for (slot, value) in values.iter().enumerate() {
        if let Some(value) = value {
            let key = computation
                .import(value.into(), &limits, counters, location)
                .unwrap();
            binding
                .set(slot, &key, &limits, counters, location)
                .unwrap();
        }
    }
    binding
}

/// Prepare actual source fixtures without manufacturing a completion witness.
pub(crate) fn prepare(text: &str) -> crate::formula::Preparation {
    let mut catalog = SupportCatalog::default();
    let mut counters = Counters::default();
    let mut budget = crate::expansion::Budget::new(crate::ExpansionLimits::default(), usize::MAX);
    let (program, location) = prepare_into(text, &mut catalog, &mut counters, &mut budget);
    crate::formula::Preparation {
        catalog,
        accounting: counters.into_accounting(),
        program,
        budget,
        limits: FormulaLimits::default(),
        options: crate::grounding_options::Execution::default(),
        location,
    }
}

/// Complete source support and lend its canonical query and computation owners.
pub(crate) fn with_completed_source(
    text: &str,
    run: impl FnOnce(
        &crate::formula_ir::Prepared,
        &Support<'_>,
        &mut Computation<'_, '_>,
        &mut Counters,
    ),
) {
    let preparation = prepare(text);
    let program = preparation.program;
    let limits = preparation.limits;
    let location = preparation.location;
    let mut budget = preparation.budget;
    let mut counters = Counters::resume(
        preparation.accounting,
        crate::grounding_observer::Work::default(),
    );
    let mut catalog = super::build(
        preparation.catalog,
        &program,
        None,
        &limits,
        &mut budget,
        &mut counters,
        location,
    )
    .unwrap();
    let (completed, mut append) = catalog.split(&limits, &mut counters, location).unwrap();
    let queries = completed
        .queries(crate::JoinStrategy::Indexed, &limits, &counters, location)
        .unwrap();
    let mut computation = Computation::new(&mut append, queries.support());
    run(&program, queries.support(), &mut computation, &mut counters);
}

fn prepare_into(
    text: &str,
    catalog: &mut SupportCatalog,
    counters: &mut Counters,
    budget: &mut crate::expansion::Budget,
) -> (crate::formula_ir::Prepared, Location) {
    let parsed = crate::ParsedSource::new(text.into(), crate::AdmissionOptions::default()).unwrap();
    let mut choices = crate::formula_choice_source::Catalog::default();
    let mut metadata = crate::metadata::Builder::default();
    let raised =
        crate::formula_choice_source::raise(parsed.parsed(), &mut metadata, budget, &mut choices)
            .unwrap();
    let location = Location {
        source: parsed.source().id(),
        span: parsed.source().span(),
    };
    let metadata = metadata.finish(location).unwrap();
    let prepared = crate::formula_ir::PreparationContext {
        options: crate::AdmissionOptions::default(),
        budget,
        catalog,
        work: super::GroundingWork::new(&FormulaLimits::default(), counters, location),
    }
    .prepare(&raised, metadata.project_selection().clone(), &choices)
    .unwrap();
    (prepared, location)
}

/// An expansion budget under the default limits, for any number of core
/// templates.
pub(crate) fn budget() -> Budget {
    Budget::new(ExpansionLimits::default(), usize::MAX)
}

/// `values` as number values.
pub(crate) fn numbers(values: &[i32]) -> Vec<Value> {
    values.iter().map(|&value| Value::Number(value)).collect()
}
