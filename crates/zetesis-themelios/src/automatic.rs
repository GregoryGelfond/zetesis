//! Shared extended-to-formula routing over the three owned input doors.

use std::fmt;
use std::sync::Arc;

use themelios_program::program::Program;
use zetesis_cpu::{Cancellation, Stop};

use crate::{
    Admitted, AdmittedBundle, BundleAdmissionError, BundleAdmissionFailure, BundleAdmissionOptions,
    ExpansionFailure, ExpansionLimits, FormulaBundleFailure, FormulaFailure, FormulaLimits,
    ParsedSource, PreparedFormula, PreparedFormulaBundle, PreparedRelational,
    ProgramFormulaOptions, ProgramRelationalOptions, SourceBundle, SourceFailure, bundle_admission,
    extended, formula, prepare_program_formula_with, prepare_program_relational,
};

pub(crate) mod handoff;

/// The first accepted profile of an automatic admission. Extended admission
/// produces relational templates; formula admission prepares later materialization.
#[derive(Debug)]
pub enum AutomaticAdmission<E, F> {
    /// The extended relational profile accepted the input.
    Extended(E),
    /// A profile-only extended refusal led to formula preparation.
    Formula(F),
}

/// The refusal from the attempted profile, retaining that door's original owner.
#[derive(Debug)]
pub enum AutomaticAdmissionFailure<E, F> {
    /// Extended admission refused without an eligible formula fallback.
    Extended(E),
    /// Formula preparation refused after a profile-only extended refusal.
    Formula(F),
}

impl<E: fmt::Display, F: fmt::Display> fmt::Display for AutomaticAdmissionFailure<E, F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Extended(error) => error.fmt(formatter),
            Self::Formula(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, F: std::error::Error + 'static> std::error::Error
    for AutomaticAdmissionFailure<E, F>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Extended(error) => Some(error),
            Self::Formula(error) => Some(error),
        }
    }
}

impl AutomaticAdmissionFailure<SourceFailure<ExpansionFailure>, SourceFailure<FormulaFailure>> {
    /// Shared cancellation or deadline, excluding resource and input refusals.
    #[must_use]
    pub fn interruption(&self) -> Option<Stop> {
        match self {
            Self::Extended(error) => expansion_interruption(error.error()),
            Self::Formula(error) => error.error().interruption(),
        }
    }
}

impl AutomaticAdmissionFailure<BundleAdmissionFailure, FormulaBundleFailure> {
    /// Shared cancellation or deadline, excluding resource and input refusals.
    #[must_use]
    pub fn interruption(&self) -> Option<Stop> {
        match self {
            Self::Extended(error) => bundle_interruption(error),
            Self::Formula(error) => error.error().interruption(),
        }
    }
}

impl AutomaticAdmissionFailure<FormulaFailure, FormulaFailure> {
    /// Shared cancellation or deadline, excluding resource and input refusals.
    #[must_use]
    pub fn interruption(&self) -> Option<Stop> {
        match self {
            Self::Extended(error) | Self::Formula(error) => error.interruption(),
        }
    }
}

/// Each door retains its native owner and failure type. Only this sequence
/// decides whether an extended refusal permits a formula attempt.
trait Door {
    type Extended;
    type Formula;
    type ExtendedFailure;
    type FormulaFailure;
    type Retry;

    fn extended(self) -> (Result<Self::Extended, Self::ExtendedFailure>, Self::Retry);
    fn interruption(error: &Self::ExtendedFailure) -> Option<Stop>;
    fn needs_formula(error: &Self::ExtendedFailure) -> bool;
    fn formula(
        retry: Self::Retry,
        error: Self::ExtendedFailure,
    ) -> Result<Self::Formula, Self::FormulaFailure>;
}

type DoorResult<D> = Result<
    AutomaticAdmission<<D as Door>::Extended, <D as Door>::Formula>,
    AutomaticAdmissionFailure<<D as Door>::ExtendedFailure, <D as Door>::FormulaFailure>,
>;

fn admit<D: Door>(door: D) -> DoorResult<D> {
    let (extended, retry) = door.extended();
    match extended {
        Ok(admitted) => Ok(AutomaticAdmission::Extended(admitted)),
        Err(error) if D::interruption(&error).is_none() && D::needs_formula(&error) => {
            D::formula(retry, error)
                .map(AutomaticAdmission::Formula)
                .map_err(AutomaticAdmissionFailure::Formula)
        }
        Err(error) => Err(AutomaticAdmissionFailure::Extended(error)),
    }
}

pub(crate) fn admit_source(
    source: ParsedSource,
    expansion: ExpansionLimits,
    formula: &FormulaLimits,
    cancellation: &Cancellation,
) -> Result<
    AutomaticAdmission<Admitted, PreparedFormula>,
    AutomaticAdmissionFailure<SourceFailure<ExpansionFailure>, SourceFailure<FormulaFailure>>,
> {
    admit(SourceDoor {
        source,
        options: SourceOptions {
            expansion,
            formula: *formula,
            cancellation: cancellation.clone(),
        },
    })
}

struct SourceOptions {
    expansion: ExpansionLimits,
    formula: FormulaLimits,
    cancellation: Cancellation,
}

struct SourceDoor {
    source: ParsedSource,
    options: SourceOptions,
}

impl Door for SourceDoor {
    type Extended = Admitted;
    type Formula = PreparedFormula;
    type ExtendedFailure = SourceFailure<ExpansionFailure>;
    type FormulaFailure = SourceFailure<FormulaFailure>;
    type Retry = (SourceOptions, handoff::Recorder);

    fn extended(self) -> (Result<Self::Extended, Self::ExtendedFailure>, Self::Retry) {
        let mut recorder = handoff::Recorder::new(self.options.formula.metadata_storage);
        let result = extended::admit_parsed_recording(
            self.source,
            self.options.expansion,
            Some(self.options.cancellation.clone()),
            Some(&mut recorder),
        );
        (result, (self.options, recorder))
    }

    fn interruption(error: &Self::ExtendedFailure) -> Option<Stop> {
        expansion_interruption(error.error())
    }

    fn needs_formula(error: &Self::ExtendedFailure) -> bool {
        error.error().needs_formula_admission()
    }

    fn formula(
        (options, recorder): Self::Retry,
        error: Self::ExtendedFailure,
    ) -> Result<Self::Formula, Self::FormulaFailure> {
        formula::prepare_parsed_reusing(
            error.into_source(),
            options.expansion,
            &options.formula,
            Some(options.cancellation),
            recorder.into_handoff(),
        )
    }
}

/// Admit an original bundle through the extended profile, preparing formulas
/// only after a refusal explicitly eligible for that profile. Original bytes,
/// include identities and diagnostics remain owned by the selected result.
/// Each source is raised once when a completed extended attempt can be reused.
/// Formula domain analysis remains an explicit opt-in on the preparation.
///
/// # Errors
/// Retains the original bundle with the first applicable refusal. Cancellation,
/// resource limits and malformed input never trigger a fallback. The same token
/// and limits govern both attempts, including deferred materialization.
pub fn admit_bundle_automatic_with_cancellation(
    bundle: SourceBundle,
    options: BundleAdmissionOptions,
    expansion: ExpansionLimits,
    formula: FormulaLimits,
    cancellation: &Cancellation,
) -> Result<
    AutomaticAdmission<AdmittedBundle, PreparedFormulaBundle>,
    AutomaticAdmissionFailure<BundleAdmissionFailure, FormulaBundleFailure>,
> {
    admit(BundleDoor {
        bundle,
        admission: options,
        options: SourceOptions {
            expansion,
            formula,
            cancellation: cancellation.clone(),
        },
    })
}

struct BundleDoor {
    bundle: SourceBundle,
    admission: BundleAdmissionOptions,
    options: SourceOptions,
}

impl Door for BundleDoor {
    type Extended = AdmittedBundle;
    type Formula = PreparedFormulaBundle;
    type ExtendedFailure = BundleAdmissionFailure;
    type FormulaFailure = FormulaBundleFailure;
    type Retry = (BundleAdmissionOptions, SourceOptions, handoff::Recorder);

    fn extended(self) -> (Result<Self::Extended, Self::ExtendedFailure>, Self::Retry) {
        let mut recorder = handoff::Recorder::new(self.options.formula.metadata_storage);
        let result = bundle_admission::admit_bundle_recording(
            self.bundle,
            self.admission,
            self.options.expansion,
            Some(self.options.cancellation.clone()),
            Some(&mut recorder),
        );
        (result, (self.admission, self.options, recorder))
    }

    fn interruption(error: &Self::ExtendedFailure) -> Option<Stop> {
        bundle_interruption(error)
    }

    fn needs_formula(error: &Self::ExtendedFailure) -> bool {
        matches!(error.error(), BundleAdmissionError::Expansion(error) if error.needs_formula_admission())
    }

    fn formula(
        (admission, options, recorder): Self::Retry,
        error: Self::ExtendedFailure,
    ) -> Result<Self::Formula, Self::FormulaFailure> {
        formula::prepare_bundle_reusing(
            error.into_bundle(),
            admission,
            options.expansion,
            &options.formula,
            Some(options.cancellation),
            recorder.into_handoff(),
        )
    }
}

/// Prepare an owned logical program through the extended relational profile,
/// then formulas only when the relational refusal explicitly permits it.
/// This door never renders or reparses input. Both attempts use the same
/// admission, expansion, purpose and cancellation settings; observation and
/// metadata-storage limits come from `options.formula` for both profiles.
/// Formula domain analysis remains an explicit opt-in on the preparation.
///
/// # Errors
/// Retains the original program with the selected profile's refusal. Input,
/// resource and interruption failures never trigger the formula attempt.
pub fn prepare_program_automatic(
    program: Arc<Program>,
    options: ProgramFormulaOptions,
) -> Result<
    AutomaticAdmission<PreparedRelational, PreparedFormula>,
    AutomaticAdmissionFailure<FormulaFailure, FormulaFailure>,
> {
    admit(ProgramDoor { program, options })
}

struct ProgramDoor {
    program: Arc<Program>,
    options: ProgramFormulaOptions,
}

impl Door for ProgramDoor {
    type Extended = PreparedRelational;
    type Formula = PreparedFormula;
    type ExtendedFailure = FormulaFailure;
    type FormulaFailure = FormulaFailure;
    type Retry = Self;

    fn extended(self) -> (Result<Self::Extended, Self::ExtendedFailure>, Self::Retry) {
        let result = prepare_program_relational(
            Arc::clone(&self.program),
            ProgramRelationalOptions {
                admission: self.options.admission,
                expansion: self.options.expansion,
                observation: self.options.formula.observation,
                metadata_storage: self.options.formula.metadata_storage,
                purpose: self.options.purpose,
                cancellation: self.options.cancellation.clone(),
            },
        );
        (result, self)
    }

    fn interruption(error: &Self::ExtendedFailure) -> Option<Stop> {
        error.interruption()
    }

    fn needs_formula(error: &Self::ExtendedFailure) -> bool {
        matches!(error.cause(), FormulaFailure::Expansion(error) if error.needs_formula_admission())
    }

    fn formula(
        retry: Self::Retry,
        _error: Self::ExtendedFailure,
    ) -> Result<Self::Formula, Self::FormulaFailure> {
        prepare_program_formula_with(retry.program, retry.options)
    }
}

fn expansion_interruption(error: &ExpansionFailure) -> Option<Stop> {
    match error {
        ExpansionFailure::Interrupted {
            reason: reason @ (Stop::Cancelled | Stop::Deadline),
            ..
        } => Some(*reason),
        _ => None,
    }
}

fn bundle_interruption(error: &BundleAdmissionFailure) -> Option<Stop> {
    match error.error() {
        BundleAdmissionError::Expansion(error) => expansion_interruption(error),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;
