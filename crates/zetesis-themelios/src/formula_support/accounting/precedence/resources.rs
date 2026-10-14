//! Exhaustive classification of the grounding account's resource failures.
//! Arithmetic undefinedness and checked language-integer overflow are semantic
//! evaluation failures; host capacity overflow is a resource refusal.

use crate::{AdmissionFailure, BundleAdmissionError, ExpansionFailure, FormulaFailure};
use zetesis_core::catalog::AssignmentError;

pub(super) fn formula(error: &FormulaFailure) -> bool {
    match error {
        FormulaFailure::Program { error, .. } => formula(error),
        FormulaFailure::Logical { error, .. } => match error {
            crate::ProgramFailureKind::Limit(_) => true,
            crate::ProgramFailureKind::Compilation(error) => match error {
                crate::CompilationFailure::Profile(_) => false,
                crate::CompilationFailure::Construction(error) => construction(error),
            },
            crate::ProgramFailureKind::Core(error) => core(error),
        },
        FormulaFailure::TemplateCatalog { error, .. } => template(error),
        FormulaFailure::TermAssignment { error, .. } => assignment(error),
        FormulaFailure::ObjectiveCondition { error, .. } => condition(error),
        FormulaFailure::AtomCatalog { error, .. } => error.is_resource_refusal(),
        FormulaFailure::MetadataAllocation { .. }
        | FormulaFailure::AtomAllocation { .. }
        | FormulaFailure::Limit { .. } => true,
        FormulaFailure::SupportTable { error, .. } => table(&error.cause),
        FormulaFailure::SupportRelation { error, .. } => relation(error),
        FormulaFailure::Observation { error } => observation(error.kind()),
        FormulaFailure::Expansion(error) => expansion(error),
        FormulaFailure::Include(error) => match error.as_ref() {
            BundleAdmissionError::Expansion(error) => expansion(error),
            BundleAdmissionError::RootAlias { .. }
            | BundleAdmissionError::IncludeAlias { .. }
            | BundleAdmissionError::IncludeRedirection { .. } => false,
        },
        FormulaFailure::Aggregate { error, .. } => error.kind().is_resource_refusal(),
        FormulaFailure::Objective { error, .. } => objective(error),
        FormulaFailure::Theory { error, .. } => error.is_resource_refusal(),
        FormulaFailure::UnadmittedTerm { .. }
        | FormulaFailure::Interrupted { .. }
        | FormulaFailure::HybridUnsupported { .. }
        | FormulaFailure::SourceActivity { .. }
        | FormulaFailure::UnsafeVariable { .. }
        | FormulaFailure::UnboundArgumentInput { .. }
        | FormulaFailure::UnboundValueInput { .. }
        | FormulaFailure::CyclicValueInput { .. } => false,
    }
}

fn template(error: &zetesis_core::TemplateCatalogFailure) -> bool {
    match error {
        zetesis_core::TemplateCatalogFailure::Storage(error) => error.is_resource_refusal(),
        zetesis_core::TemplateCatalogFailure::Read(error) => read(*error),
        zetesis_core::TemplateCatalogFailure::Incomplete => false,
        zetesis_core::TemplateCatalogFailure::Stopped(never) => match *never {},
    }
}

fn read(error: zetesis_core::catalog::ReadError) -> bool {
    use zetesis_core::catalog::ReadError;
    match error {
        ReadError::ForeignCatalog
        | ReadError::OutsidePrefix
        | ReadError::Uninterned
        | ReadError::Predicate => false,
    }
}

fn assignment(error: &AssignmentError) -> bool {
    match error {
        AssignmentError::Storage(error) => error.is_resource_refusal(),
        AssignmentError::Read(error) => read(*error),
        AssignmentError::Slot { .. } | AssignmentError::Unbound { .. } => false,
    }
}

fn condition(error: &zetesis_objective::ConditionError) -> bool {
    use zetesis_objective::ConditionError;
    match error {
        ConditionError::Storage(error) => error.is_resource_refusal(),
        ConditionError::AtomReference { .. } | ConditionError::Reference { .. } => false,
    }
}

fn relation(error: &zetesis_core::relation::Failure) -> bool {
    use zetesis_core::relation::Failure;
    match error {
        Failure::Overflow | Failure::Allocation | Failure::Limit { .. } => true,
        Failure::Read(error) => read(*error),
        Failure::Predicate
        | Failure::CatalogIndex
        | Failure::Column
        | Failure::Selection
        | Failure::Mask
        | Failure::Owner
        | Failure::Dictionary
        | Failure::Order => false,
    }
}

fn table(error: &zetesis_cpu::table::Cause) -> bool {
    use zetesis_cpu::table::Cause;
    match error {
        Cause::Overflow | Cause::Allocation | Cause::Limit { .. } => true,
        Cause::Scope | Cause::Domains | Cause::Relation | Cause::Interrupted(_) => false,
    }
}

fn observation(error: &crate::observation::ErrorKind) -> bool {
    use crate::observation::ErrorKind;
    match error {
        ErrorKind::Limit { .. } | ErrorKind::Allocation => true,
        ErrorKind::Metadata(error) => metadata(error),
        ErrorKind::TermAssignment(error) => assignment(error),
        ErrorKind::TermStorage(error) => error.is_resource_refusal(),
        ErrorKind::Unsupported(_)
        | ErrorKind::Evaluation(_)
        | ErrorKind::Stopped(_)
        | ErrorKind::InvalidSymbol => false,
    }
}

fn metadata(error: &crate::MetadataStorageError) -> bool {
    match error {
        crate::MetadataStorageError::Components(error) => template(error),
        crate::MetadataStorageError::Assignment(error) => assignment(error),
    }
}

fn construction(error: &zetesis_core::ConstructionError) -> bool {
    use zetesis_core::{ConstructionError, ValueError};
    match error {
        ConstructionError::Value(ValueError::Limit { .. } | ValueError::Allocation) => true,
        ConstructionError::Value(ValueError::Shape)
        | ConstructionError::EmptyPredicateName
        | ConstructionError::ArityMismatch { .. } => false,
    }
}

fn core(error: &zetesis_core::AdmissionError) -> bool {
    use zetesis_core::AdmissionError;
    match error {
        AdmissionError::Canonical { error, .. } => error.is_resource_refusal(),
        AdmissionError::LimitExceeded { .. } | AdmissionError::FactLimitExceeded { .. } => true,
        AdmissionError::UnsafeVariable { .. } | AdmissionError::NonDenseVariable { .. } => false,
    }
}

fn admission(error: &AdmissionFailure) -> bool {
    match error {
        AdmissionFailure::Limit { .. } | AdmissionFailure::Source { .. } => true,
        AdmissionFailure::Construction { error, .. } => construction(error),
        AdmissionFailure::Metadata { error, .. } => metadata(error),
        AdmissionFailure::Core { error, .. } => core(error),
        AdmissionFailure::Syntax(_)
        | AdmissionFailure::Raise(_)
        | AdmissionFailure::Profile { .. }
        | AdmissionFailure::ExtremumEndpoint { .. } => false,
    }
}

fn expansion(error: &ExpansionFailure) -> bool {
    match error {
        ExpansionFailure::Allocation { .. } | ExpansionFailure::Limit { .. } => true,
        ExpansionFailure::Admission(error) => admission(error),
        ExpansionFailure::Interrupted { .. }
        | ExpansionFailure::DuplicateConstant { .. }
        | ExpansionFailure::ConstantPolicy { .. }
        | ExpansionFailure::ConstantCycle { .. }
        | ExpansionFailure::Evaluation { .. } => false,
    }
}

fn objective(error: &zetesis_objective::AdmissionError) -> bool {
    use zetesis_objective::AdmissionError;
    match error {
        AdmissionError::Storage { error, .. } => error.is_resource_refusal(),
        AdmissionError::ConditionStorage { error, .. } => condition(error),
        AdmissionError::Limit { .. }
        | AdmissionError::Overflow { .. }
        | AdmissionError::Allocation => true,
        AdmissionError::CatalogRow { .. }
        | AdmissionError::MissingWeight { .. }
        | AdmissionError::ConditionNotCanonical { .. }
        | AdmissionError::UnsafeVariable { .. }
        | AdmissionError::NonDenseVariable { .. }
        | AdmissionError::MissingPriority { .. }
        | AdmissionError::ConditionReference { .. } => false,
    }
}
