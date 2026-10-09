//! Native run outcomes for the closed CPU, unscored, unprojected adapter profile.
//!
//! Only request-owned cancellation/deadline tokens may enter this bridge. Native
//! resource ceilings are faults, never successful truncation. Source refusals
//! resolve the actual original statement; there is no fabricated part/statement.

use themelios_program::{Program, program::Statement, provenance::WithProvenance};
use themelios_solve::{contract::Fault, outcome::Conclusion};
use zetesis_cpu::Stop;
use zetesis_solve::{Interruption, SearchState, SemanticOutcome, SolveError, SolveFailure};
use zetesis_themelios::{FormulaFailure, ProgramSite, ProgramSubject, observation};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Control(Conclusion),
    Resource,
    Program,
    Engine,
    Adapter,
}

/// This assumes `Deadline` came from the acknowledged request, not an internal
/// timeout configured independently of that request.
pub(crate) fn control(stop: Stop) -> Option<Conclusion> {
    match stop {
        Stop::Cancelled => Some(Conclusion::Interrupted),
        Stop::Deadline => Some(Conclusion::Budget),
        _ => None,
    }
}

fn stop_class(stop: Stop) -> Class {
    match stop {
        Stop::Cancelled => Class::Control(Conclusion::Interrupted),
        Stop::Deadline => Class::Control(Conclusion::Budget),
        Stop::WorkLimit
        | Stop::RoundLimit
        | Stop::StorageLimit
        | Stop::DerivedAtomLimit
        | Stop::CandidateLimit
        | Stop::CarrierLimit
        | Stop::Allocation => Class::Resource,
        Stop::WrongProgram | Stop::InvalidProgram => Class::Engine,
    }
}

fn fault(
    class: Class,
    message: String,
    subject: Option<ProgramSubject<'_>>,
) -> Result<Conclusion, Fault> {
    match class {
        Class::Control(conclusion) => Ok(conclusion),
        Class::Resource => Err(Fault::resource(message)),
        Class::Engine => Err(Fault::engine(message)),
        Class::Adapter => Err(Fault::adapter_bug(message)),
        Class::Program => Err(match subject {
            Some(ProgramSubject::Statement(statement)) => Fault::program(message, statement),
            Some(ProgramSubject::Part(part)) => Fault::program_part(message, part),
            Some(ProgramSubject::Program) | None => Fault::adapter_bug(format!(
                "native statement refusal omitted its original subject: {message}"
            )),
        }),
    }
}

fn statement(program: &Program, site: Option<ProgramSite>) -> Option<&WithProvenance<Statement>> {
    program.statements().nth(site?.statement_id()?.index())
}

pub(crate) fn interruption(reason: Interruption) -> Result<Conclusion, Fault> {
    fault(interruption_class(reason), reason.to_string(), None)
        .map_err(|fault| fault.caused_by(reason))
}

fn interruption_class(reason: Interruption) -> Class {
    match reason {
        Interruption::Preparation(stop)
        | Interruption::Oracle(stop)
        | Interruption::Constraint(stop)
        | Interruption::Reconstruction(stop) => stop_class(stop),
        Interruption::Countermodel(error) => sat_class(error),
        Interruption::ModelConstruction(zetesis_solve::ModelConstructionStop::Control(stop)) => {
            stop_class(stop)
        }
        Interruption::ModelConstruction(
            zetesis_solve::ModelConstructionStop::Work { .. }
            | zetesis_solve::ModelConstructionStop::Bytes { .. },
        ) => Class::Resource,
        // The adapter configures AnswerSets/All, without scoring or incumbents.
        Interruption::Objective(_)
        | Interruption::PreparedObjective(_)
        | Interruption::Incumbent(_) => Class::Adapter,
    }
}

pub(crate) fn completed(outcome: Option<&SemanticOutcome>) -> Result<Conclusion, Fault> {
    match outcome.and_then(SemanticOutcome::search_state) {
        Some(SearchState::Exhausted) => Ok(Conclusion::Exhausted),
        Some(SearchState::Interrupted(reason)) => interruption(reason),
        Some(SearchState::RequestedModels) => Err(Fault::adapter_bug(
            "native requested-model stop in an uncapped adapter run",
        )),
        Some(SearchState::PendingInterruption(_)) => Err(Fault::adapter_bug(
            "native run completed with checked answers still pending",
        )),
        None => Err(Fault::adapter_bug(
            "native run completed without established search coverage",
        )),
    }
}

pub(crate) fn session(error: SolveFailure, original: &Program) -> Result<Conclusion, Fault> {
    // Semantic evidence is not a replacement for the actual error: a later
    // engine/publication failure must not become a clean semantic conclusion.
    let result = match error.cause.as_ref() {
        SolveError::Reconstruction(
            zetesis_themelios::ReconstructionError::Source(source)
            | zetesis_themelios::ReconstructionError::Model(zetesis_core::ModelFailure::Stopped(
                source,
            )),
        ) => formula_ref(source, original),
        SolveError::Constraint(failure) => match &failure.cause {
            zetesis_themelios::ConstraintCheckCause::Source(source)
            | zetesis_themelios::ConstraintCheckCause::Index(
                zetesis_core::AtomIndexError::Stopped(source),
            ) => formula_ref(source, original),
            _ => fault(constraint_class(&failure.cause), error.to_string(), None),
        },
        cause => fault(solve_class(cause), error.to_string(), None),
    };
    result.map_err(|fault| fault.caused_by(error))
}

pub(crate) fn observation(
    error: observation::Error,
    original: &Program,
) -> Result<Conclusion, Fault> {
    fault(
        observation_class(error.kind()),
        error.to_string(),
        statement(original, Some(error.site())).map(ProgramSubject::Statement),
    )
    .map_err(|fault| fault.caused_by(error))
}

fn solve_class(error: &SolveError) -> Class {
    match error {
        SolveError::TerminalStatisticsOverflow
        | SolveError::HybridStatisticsOverflow
        | SolveError::LazyStatisticsOverflow
        | SolveError::ClosureStatisticsOverflow
        | SolveError::ModelStatisticsOverflow
        | SolveError::ClosureReservation { .. }
        | SolveError::CompletionPool(_) => Class::Resource,
        SolveError::Batch(error) => batch_class(error),
        SolveError::QueryObservation(error) => batch_class(error),
        SolveError::SharedCpu(error) => match error {
            zetesis_cpu::lazy::shared::Cause::Source(stop)
            | zetesis_cpu::lazy::shared::Cause::World { stop, .. } => stop_class(*stop),
            zetesis_cpu::lazy::shared::Cause::InvalidOutput => Class::Engine,
        },
        SolveError::Static(error) => static_class(error),
        SolveError::Words(zetesis_core::WordError::Model(error)) | SolveError::Model(error) => {
            model_class(error)
        }
        SolveError::Reconstruction(zetesis_themelios::ReconstructionError::Model(
            zetesis_core::ModelFailure::Model(error),
        )) => model_class(error),
        SolveError::ExecutionObservation(_)
        | SolveError::ConstraintFailureMissing
        | SolveError::Executor(_)
        | SolveError::Words(_)
        | SolveError::Reconstruction(_)
        | SolveError::Constraint(_)
        | SolveError::CandidateStreamNotExhausted
        | SolveError::FormulaBatchShape { .. } => Class::Engine,
        // The remaining CPU variants concern an incompatible configured profile:
        // Projection, HybridBackend, BackendUnavailable, UnsupportedOracle,
        // UnsupportedSourceBatching or PreparedInput. Cargo feature unification
        // can also expose Gpu/LazyGpu errors without a matching adapter feature.
        // This closed CPU, unprojected profile admits none of those paths. Any
        // unclassified variant is a bridge configuration breach, never a clean
        // conclusion or a guessed engine/resource classification. Widening the
        // profile requires explicit classification of the newly reachable errors.
        _ => Class::Adapter,
    }
}

fn batch_class(error: &zetesis_cpu::BatchError) -> Class {
    match error {
        zetesis_cpu::BatchError::Preparation(stop) => stop_class(*stop),
        zetesis_cpu::BatchError::ClosureStorage { .. }
        | zetesis_cpu::BatchError::Capacity { .. }
        | zetesis_cpu::BatchError::Pool(_) => Class::Resource,
        zetesis_cpu::BatchError::Busy | zetesis_cpu::BatchError::Poisoned => Class::Engine,
    }
}

fn sat_class(error: zetesis_sat::Incomplete) -> Class {
    use zetesis_sat::Incomplete as E;
    match error {
        E::Cancelled => Class::Control(Conclusion::Interrupted),
        E::Deadline => Class::Control(Conclusion::Budget),
        E::Verification(stop) => stop_class(stop),
        E::Admission(error) => sat_admission_class(error),
        E::Certificate(error) => certificate_class(error),
        E::WorkLimit
        | E::DecisionLimit
        | E::CandidateLimit
        | E::PendingBytes
        | E::CompletionScratch
        | E::ReductStorage { .. }
        | E::BatchCandidateLimit
        | E::Allocation
        | E::ProjectionLimit { .. }
        | E::CounterOverflow => Class::Resource,
        E::WorkerPanicked
        | E::PendingBatch
        | E::WrongTheory
        | E::RestrictionUniverse { .. }
        | E::ClosedEnumerator
        | E::LateCertificate
        | E::RegionFilter
        | E::InvalidRegionConsequence
        | E::RegionFilterUnsupported
        | E::LateRegionFilter
        | E::RegionFilterAlreadySet
        | E::InvalidWitness => Class::Engine,
    }
}

fn sat_admission_class(error: zetesis_sat::AdmissionError) -> Class {
    match error {
        zetesis_sat::AdmissionError::Limit { .. }
        | zetesis_sat::AdmissionError::Overflow
        | zetesis_sat::AdmissionError::Allocation => Class::Resource,
        zetesis_sat::AdmissionError::Variable { .. } => Class::Engine,
    }
}

fn certificate_class(error: zetesis_sat::CertificateError) -> Class {
    use zetesis_ferraris::{EvaluationError, PositiveError, StratifiedError, TightError};
    use zetesis_sat::CertificateError;
    match error {
        CertificateError::Restriction(error) => sat_admission_class(error),
        CertificateError::Evaluation(EvaluationError::Stopped(stop))
        | CertificateError::Tight(TightError::Stopped(stop))
        | CertificateError::Positive(PositiveError::Stopped(stop))
        | CertificateError::Stratified(StratifiedError::Stopped(stop)) => stop_class(stop),
        CertificateError::Evaluation(EvaluationError::Storage { .. })
        | CertificateError::Tight(TightError::Limit(_))
        | CertificateError::Positive(PositiveError::Limit { .. } | PositiveError::Overflow)
        | CertificateError::Stratified(StratifiedError::Limit { .. } | StratifiedError::Overflow) => {
            Class::Resource
        }
        // Ordinary ineligibility must decline the optional native optimization;
        // it cannot terminate the adapter's otherwise supported answer search.
        CertificateError::Tight(_)
        | CertificateError::Positive(_)
        | CertificateError::Stratified(_) => Class::Engine,
    }
}

fn constraint_class(error: &zetesis_themelios::ConstraintCheckCause) -> Class {
    match error {
        zetesis_themelios::ConstraintCheckCause::Stopped(stop) => stop_class(*stop),
        zetesis_themelios::ConstraintCheckCause::Index(
            zetesis_core::AtomIndexError::Allocation,
        ) => Class::Resource,
        _ => Class::Engine,
    }
}

fn static_class(error: &zetesis_core::StaticError) -> Class {
    use zetesis_core::{CarrierError, StaticError};
    match error {
        StaticError::CountOverflow
        | StaticError::DenseIdOverflow
        | StaticError::LimitExceeded { .. }
        | StaticError::Allocation
        | StaticError::Carrier(CarrierError::Allocation | CarrierError::Bytes { .. }) => {
            Class::Resource
        }
        StaticError::Catalog(error) => catalog_class(error),
        StaticError::Carrier(CarrierError::Coordinates) | StaticError::InvalidAdmittedProgram => {
            Class::Engine
        }
    }
}

fn model_class(error: &zetesis_core::ModelError) -> Class {
    match error {
        zetesis_core::ModelError::Bytes { .. } | zetesis_core::ModelError::Allocation => {
            Class::Resource
        }
        zetesis_core::ModelError::Catalog(error) => catalog_class(error),
        zetesis_core::ModelError::Order { .. } | zetesis_core::ModelError::Position { .. } => {
            Class::Engine
        }
    }
}

fn catalog_class(error: &zetesis_core::catalog::Error) -> Class {
    use zetesis_core::{ValueError, catalog::Error};
    match error {
        Error::Value(ValueError::Limit { .. } | ValueError::Allocation)
        | Error::Allocation
        | Error::Storage { .. }
        | Error::IdExhausted
        | Error::Overflow => Class::Resource,
        Error::Value(ValueError::Shape)
        | Error::Shape
        | Error::FrozenVocabulary
        | Error::VocabularyHasAtoms
        | Error::UnindexedVocabulary
        | Error::CatalogHasBase => Class::Engine,
    }
}

fn assignment_class(error: &zetesis_core::catalog::AssignmentError) -> Class {
    match error {
        zetesis_core::catalog::AssignmentError::Storage(error) => catalog_class(error),
        _ => Class::Engine,
    }
}

fn template_class(error: &zetesis_core::TemplateCatalogFailure) -> Class {
    match error {
        zetesis_core::TemplateCatalogFailure::Storage(error) => catalog_class(error),
        zetesis_core::TemplateCatalogFailure::Stopped(never) => match *never {},
        _ => Class::Engine,
    }
}

fn metadata_class(error: &zetesis_themelios::MetadataStorageError) -> Class {
    match error {
        zetesis_themelios::MetadataStorageError::Components(error) => template_class(error),
        zetesis_themelios::MetadataStorageError::Assignment(error) => assignment_class(error),
    }
}

fn observation_class(error: &observation::ErrorKind) -> Class {
    match error {
        observation::ErrorKind::Stopped(stop) => stop_class(*stop),
        observation::ErrorKind::Limit { .. } | observation::ErrorKind::Allocation => {
            Class::Resource
        }
        observation::ErrorKind::Metadata(error) => metadata_class(error),
        observation::ErrorKind::TermStorage(error) => catalog_class(error),
        observation::ErrorKind::TermAssignment(error) => assignment_class(error),
        observation::ErrorKind::Unsupported(_) | observation::ErrorKind::Evaluation(_) => {
            Class::Program
        }
        observation::ErrorKind::InvalidSymbol => Class::Engine,
    }
}

/// Structural validation carries borrowed evidence and an owned typed cause.
/// It has no cancellation or deadline effect; its classifier cannot return control.
pub(crate) fn admission(error: zetesis_themelios::ProgramAdmissionFailure<'_>) -> Fault {
    let class = program_class(&error.kind);
    fault(class, error.to_string(), Some(error.subject))
        .expect_err("structural admission has no runtime control")
        .caused_by(error.kind)
}

pub(crate) fn formula(error: FormulaFailure, original: &Program) -> Result<Conclusion, Fault> {
    formula_ref(&error, original).map_err(|fault| fault.caused_by(error))
}

fn formula_ref(error: &FormulaFailure, original: &Program) -> Result<Conclusion, Fault> {
    let subject = error
        .subject()
        .or_else(|| statement(original, error.site()).map(ProgramSubject::Statement));
    // Whole-input validation limits are resource faults. A semantic refusal
    // must identify its actual statement or excluded program part; absent
    // evidence is an internal invariant failure, never a guessed subject.
    fault(formula_class(error), error.to_string(), subject)
}

fn program_class(error: &zetesis_themelios::ProgramFailureKind) -> Class {
    match error {
        zetesis_themelios::ProgramFailureKind::Limit(_) => Class::Resource,
        zetesis_themelios::ProgramFailureKind::Core(error) => core_admission_class(error),
        zetesis_themelios::ProgramFailureKind::Compilation(error) => match error {
            zetesis_themelios::CompilationFailure::Profile(_) => Class::Program,
            zetesis_themelios::CompilationFailure::Construction(error) => construction_class(error),
        },
    }
}

fn formula_class(error: &FormulaFailure) -> Class {
    use zetesis_themelios::FormulaFailure as E;
    match error.cause() {
        E::Limit { .. } | E::MetadataAllocation { .. } | E::AtomAllocation { .. } => {
            Class::Resource
        }
        E::Interrupted { reason, .. } => stop_class(*reason),
        E::AtomCatalog { error, .. } => catalog_class(error),
        E::TermAssignment { error, .. } => assignment_class(error),
        E::TemplateCatalog { error, .. } => template_class(error),
        E::Observation { error } => observation_class(error.kind()),
        E::Expansion(error) => expansion_class(error),
        E::Aggregate { error, .. } => aggregate_class(*error),
        E::Theory { error, .. } => theory_class(*error),
        E::SupportRelation { error, .. } => relation_class(error),
        E::SupportTable { error, .. } => table_class(&error.cause),
        E::Logical { error, .. } => program_class(error),
        E::HybridUnsupported { .. }
        | E::UnsafeVariable { .. }
        | E::UnboundArgumentInput { .. }
        | E::UnboundValueInput { .. }
        | E::CyclicValueInput { .. } => Class::Program,
        E::UnadmittedTerm { .. } | E::SourceActivity { .. } => Class::Engine,
        E::ObjectiveCondition { .. } | E::Objective { .. } | E::Include(_) | E::Program { .. } => {
            Class::Adapter
        }
    }
}

fn expansion_class(error: &zetesis_themelios::ExpansionFailure) -> Class {
    use zetesis_themelios::ExpansionFailure as E;
    match error {
        E::Interrupted { reason, .. } => stop_class(*reason),
        E::Limit { .. } => Class::Resource,
        E::Admission(error) => admission_class(error),
        E::DuplicateConstant { .. }
        | E::ConstantPolicy { .. }
        | E::ConstantCycle { .. }
        | E::Evaluation { .. } => Class::Program,
    }
}

fn admission_class(error: &zetesis_themelios::AdmissionFailure) -> Class {
    use zetesis_themelios::AdmissionFailure as E;
    match error {
        E::Limit { .. } | E::Source { .. } => Class::Resource,
        E::Metadata { error, .. } => metadata_class(error),
        E::Core { error, .. } => core_admission_class(error),
        E::Construction { error, .. } => construction_class(error),
        E::Profile { .. } | E::ExtremumEndpoint { .. } => Class::Program,
        E::Syntax(_) | E::Raise(_) => Class::Adapter,
    }
}

fn construction_class(error: &zetesis_core::ConstructionError) -> Class {
    match error {
        zetesis_core::ConstructionError::Value(
            zetesis_core::ValueError::Limit { .. } | zetesis_core::ValueError::Allocation,
        ) => Class::Resource,
        _ => Class::Program,
    }
}

fn core_admission_class(error: &zetesis_core::AdmissionError) -> Class {
    match error {
        zetesis_core::AdmissionError::Canonical { error, .. } => catalog_class(error),
        zetesis_core::AdmissionError::LimitExceeded { .. } => Class::Resource,
        zetesis_core::AdmissionError::UnsafeVariable { .. } => Class::Program,
        zetesis_core::AdmissionError::NonDenseVariable { .. } => Class::Engine,
    }
}

fn aggregate_class(error: zetesis_ferraris::AggregateError) -> Class {
    use zetesis_ferraris::AggregateErrorKind as E;
    match error.kind() {
        E::Control(stop) => stop_class(stop),
        E::InvalidPrefix { .. } | E::InvalidCondition { .. } => Class::Engine,
        E::ElementLimit
        | E::GuardLimit
        | E::NodeLimit
        | E::OperandLimit
        | E::WorkLimit
        | E::StateLimit
        | E::SubsetLimit
        | E::Allocation
        | E::ArithmeticOverflow => Class::Resource,
    }
}

fn theory_class(error: zetesis_ferraris::AdmissionError) -> Class {
    match error {
        zetesis_ferraris::AdmissionError::Limit | zetesis_ferraris::AdmissionError::Allocation => {
            Class::Resource
        }
        zetesis_ferraris::AdmissionError::Atom
        | zetesis_ferraris::AdmissionError::Edge
        | zetesis_ferraris::AdmissionError::Root
        | zetesis_ferraris::AdmissionError::Arity
        | zetesis_ferraris::AdmissionError::Span => Class::Engine,
    }
}

fn relation_class(error: &zetesis_core::relation::Failure) -> Class {
    match error {
        zetesis_core::relation::Failure::Overflow
        | zetesis_core::relation::Failure::Allocation
        | zetesis_core::relation::Failure::Limit { .. } => Class::Resource,
        _ => Class::Engine,
    }
}

fn table_class(error: &zetesis_cpu::table::Cause) -> Class {
    match error {
        zetesis_cpu::table::Cause::Interrupted(stop) => stop_class(*stop),
        zetesis_cpu::table::Cause::Limit { .. }
        | zetesis_cpu::table::Cause::Overflow
        | zetesis_cpu::table::Cause::Allocation => Class::Resource,
        _ => Class::Engine,
    }
}

#[cfg(test)]
mod tests;
