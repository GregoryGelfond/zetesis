//! A formula's truth in an interpretation and in the reduct by it, computed
//! from its connectives alone.

/// A formula's truth in an interpretation M (`whole`) and, for a J within M,
/// whether J satisfies the formula's reduct by M (`frozen`).
#[derive(Clone, Copy)]
pub(crate) struct Truth {
    pub(crate) whole: bool,
    pub(crate) frozen: bool,
}

impl Truth {
    /// A constant, true or false in both.
    pub(crate) fn constant(value: bool) -> Self {
        Self {
            whole: value,
            frozen: value,
        }
    }

    /// An atom in M (`whole`) and in J (`inner`); the reduct keeps it only
    /// when it is in M.
    pub(crate) fn atom(whole: bool, inner: bool) -> Self {
        Self {
            whole,
            frozen: whole && inner,
        }
    }

    pub(crate) fn and(self, other: Self) -> Self {
        Self {
            whole: self.whole && other.whole,
            frozen: self.frozen && other.frozen,
        }
    }

    pub(crate) fn or(self, other: Self) -> Self {
        Self {
            whole: self.whole || other.whole,
            frozen: self.frozen || other.frozen,
        }
    }

    /// An implication, which the reduct replaces by falsum when it is false
    /// in M.
    pub(crate) fn implies(self, other: Self) -> Self {
        let whole = !self.whole || other.whole;
        Self {
            whole,
            frozen: whole && (!self.frozen || other.frozen),
        }
    }

    /// Default negation, as an implication of falsum.
    pub(crate) fn negate(self) -> Self {
        self.implies(Self::constant(false))
    }

    /// A choice of the formula: it or its negation.
    pub(crate) fn choice(self) -> Self {
        self.or(self.negate())
    }
}
