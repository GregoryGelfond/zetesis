//! Ordinary execution policies, independent of argument parsing and presentation.
//!
//! The execution backend is the shared vocabulary [`crate::Backend`].

/// Candidate proposal scheduling for general and tight GPU formula checks.
/// CPU execution keeps its configured batch size.
/// The ordinary policy, [`Self::DEFAULT`], is [`Self::Adaptive`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GpuBatching {
    /// Propose every slate, the candidates checked and committed together, at
    /// [`crate::SolveConfig::batch_size`]; for controlled comparisons.
    Fixed,
    /// Slates start at [`crate::SolveConfig::batch_size`]. The size of the next
    /// slate is chosen when it is selected, after the previous slate committed
    /// completely and its answers were scored: it returns to the batch size
    /// when those answers tightened the objective bound, doubles after a filled
    /// slate that left the bound unchanged, and otherwise stays. Growth stops at
    /// the smallest device prefix observed, never below the batch size, and the
    /// pending-memory allowance bounds every slate. A retried slate keeps its
    /// count. Under either policy a device submission takes as much of a slate
    /// as the device admits. A larger slate releases its answers only after the
    /// whole slate is checked, and is produced under the bound as it stood when
    /// it started; membership, optima and partial-publication rules are
    /// unchanged.
    Adaptive,
}

impl GpuBatching {
    /// The ordinary policy, [`Self::Adaptive`]; [`Default`] returns it.
    pub const DEFAULT: Self = Self::Adaptive;
}

impl Default for GpuBatching {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How an optimizing search proves its optimum and finds that optimum's ties.
/// The ordinary policy, [`Self::DEFAULT`], is [`Self::StrictForOneAnswer`].
/// Every policy returns the same optimum and the same optimal answer sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum OptimumSearch {
    /// A request for one optimal answer bounds the candidates strictly, as
    /// [`Self::StrictWithTiePass`] does, where no tie is owed; a request for
    /// more is [`Self::Inclusive`]. A strict bound meets no tie of an
    /// incumbent it has beaten, so one answer is proved with less search; a
    /// tie pass from the root can cost more than one inclusive pass does.
    StrictForOneAnswer,
    /// Each strictly better answer bounds the candidates to cost strictly
    /// less than its own, so the search meets no tie of an incumbent it has
    /// already beaten. Exhausting that search proves the optimum. When more
    /// than one optimal answer is requested, and the last installed bound
    /// pruned the optimum's ties, one more pass over the same theory, bounded
    /// to cost at most the optimum, enumerates them, each once. A stop of that
    /// pass leaves the optimum proved and the optimal family incomplete.
    StrictWithTiePass,
    /// Each strictly better answer bounds the candidates to cost at most its
    /// own, so one pass proves the optimum and meets every tie along the way.
    /// The schedule of earlier releases, for controlled comparisons.
    Inclusive,
}

impl OptimumSearch {
    /// The ordinary policy, [`Self::StrictForOneAnswer`]; [`Default`] returns it.
    pub const DEFAULT: Self = Self::StrictForOneAnswer;
}

impl Default for OptimumSearch {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Materialization policy, independent of execution hardware.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grounder {
    /// Select the input's certified execution policy. A complete acyclic normal
    /// source can evaluate directly on CPU; other profiles retain their eager
    /// base or terminal reconstruction.
    #[default]
    Auto,
    /// Instantiate on demand: a producer core with eligible constraints
    /// streamed and terminal definitions reconstructed per answer. With a GPU
    /// backend the hybrid schedule checks its retained core on the device and
    /// streamed constraints on the host.
    Lazy,
    /// Instantiate every formula before checking on CPU or GPU.
    Eager,
}

impl Grounder {
    /// Stable spelling for configuration and execution reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Lazy => "lazy",
            Self::Eager => "eager",
        }
    }
}

/// Exact stable-model oracle selection, independent of language support.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Oracle {
    /// Select checked tight support or general reduct checking.
    #[default]
    Auto,
    /// Require eager Ferraris search: CPU, or GPU propagation with exact CPU residuals.
    Countermodel,
}

impl Oracle {
    /// Stable spelling for configuration and execution reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Countermodel => "countermodel",
        }
    }
}
