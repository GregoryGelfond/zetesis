use std::fmt;

/// Per-model ceilings, independent of source admission and stable-model search.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Charged join, equality, tuple, key-comparison, shift and accounting work.
    pub max_work: u64,
    /// Complete positive joins visited, including bindings rejected by filters.
    pub max_bindings: u64,
    /// Distinct global contribution keys retained.
    pub max_keys: usize,
    /// Canonical encoded payload bytes of distinct keys; not allocator overhead.
    pub max_key_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_work: 10_000_000,
            max_bindings: 1_000_000,
            max_keys: 1_000_000,
            max_key_bytes: 67_108_864,
        }
    }
}

/// Exact accounting from an evaluation, also retained on failure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Charged operations, including initialization and final cost conversion.
    pub work: u64,
    /// Complete positive joins visited before filters.
    pub bindings: u64,
    /// Bindings whose scalar filters passed.
    pub active_bindings: u64,
    /// Repeated contribution keys coalesced across bindings or templates.
    pub duplicates: u64,
    /// Distinct contribution keys retained.
    pub keys: usize,
    /// Retained canonical key payload bytes.
    pub key_bytes: usize,
}

/// An interrupted evaluation, never a proof about optimum or satisfiability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Shared cancellation was observed.
    Cancelled,
    /// The absolute deadline expired.
    Deadline,
    /// The charged operation ceiling was reached.
    WorkLimit,
    /// The complete-binding ceiling was reached.
    BindingLimit,
    /// Adding another unique key would exceed the count ceiling.
    KeyLimit,
    /// Adding another unique key would exceed the payload-byte ceiling.
    KeyBytesLimit,
    /// Fallible storage reservation failed.
    Allocation,
    /// Size or accounting arithmetic cannot be represented on this host.
    ArithmeticOverflow,
    /// An unexpected shared-control refusal, retained without reclassification.
    Control(zetesis_cpu::Stop),
}
impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("objective evaluation cancelled"),
            Self::Deadline => f.write_str("objective evaluation deadline expired"),
            Self::WorkLimit => f.write_str("objective work limit reached"),
            Self::BindingLimit => f.write_str("objective binding limit reached"),
            Self::KeyLimit => f.write_str("objective contribution-key limit reached"),
            Self::KeyBytesLimit => f.write_str("objective key-byte limit reached"),
            Self::Allocation => f.write_str("objective storage reservation failed"),
            Self::ArithmeticOverflow => f.write_str("objective accounting arithmetic overflow"),
            Self::Control(reason) => reason.fmt(f),
        }
    }
}
impl std::error::Error for Stop {}

/// The typed reason an evaluator cannot return a complete score.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// A control, work, storage or representation bound stopped evaluation.
    Stopped(Stop),
    /// A negated numeric weight is outside the signed 32-bit contribution range.
    WeightNormalizationOverflow,
    /// The exact total at one priority is outside the signed 64-bit score range.
    CostOverflow {
        /// The affected priority.
        priority: i32,
    },
    /// Admitted safety was violated internally while resolving a local variable.
    UnboundVariable {
        /// The unbound local variable index.
        variable: usize,
    },
}

/// An evaluation failure with source-template correlation and partial statistics.
/// No partial objective score is exposed as a successful result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub(crate) kind: ErrorKind,
    pub(crate) template: Option<usize>,
    pub(crate) statistics: Statistics,
}
impl Error {
    /// The structured refusal or stopped resource.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }
    /// Original template index for the caller's separate source-origin catalog.
    /// Final cross-template cost overflow has no single originating template.
    #[must_use]
    pub const fn template_index(&self) -> Option<usize> {
        self.template
    }
    /// Work completed before the failure; these counts imply no complete score.
    #[must_use]
    pub const fn statistics(&self) -> Statistics {
        self.statistics
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ErrorKind::Stopped(reason) => reason.fmt(f)?,
            ErrorKind::WeightNormalizationOverflow => {
                f.write_str("objective numeric weight negation exceeds signed 32-bit range")?;
            }
            ErrorKind::CostOverflow { priority } => write!(
                f,
                "objective cost at priority {priority} exceeds signed 64-bit range"
            )?,
            ErrorKind::UnboundVariable { variable } => {
                write!(f, "objective evaluation lacks local variable {variable}")?;
            }
        }
        if let Some(template) = self.template {
            write!(f, " (template {template})")?;
        }
        Ok(())
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::Stopped(reason) => Some(reason),
            _ => None,
        }
    }
}
