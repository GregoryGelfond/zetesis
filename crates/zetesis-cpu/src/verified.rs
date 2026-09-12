//! Stable interpretations produced by completed native closure checks.

use zetesis_core::{Interpretation, Program};

/// A stable interpretation bound to the exact admitted program that was checked.
/// Native closure acceptance is the only construction path. This establishes
/// membership for one interpretation, not enumeration coverage. Accessors borrow
/// without allocation; consuming the receipt transfers the owned interpretation.
/// Cloning shares the program, atom catalog and selected interpretation.
/// The receipt records a native runtime result; it is not a formal proof object.
///
/// ```compile_fail
/// use zetesis_core::{Interpretation, Program};
/// use zetesis_cpu::StableInterpretation;
/// fn forge(program: Program, interpretation: Interpretation) -> StableInterpretation {
///     StableInterpretation { program, interpretation }
/// }
/// ```
#[derive(Clone, Debug)]
pub struct StableInterpretation {
    program: Program,
    interpretation: Interpretation,
}

impl StableInterpretation {
    pub(crate) fn new(program: Program, interpretation: Interpretation) -> Self {
        Self {
            program,
            interpretation,
        }
    }

    /// The immutable admitted instance for which stable membership was established.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }

    /// The exact stable atom set. Borrowing does not erase this receipt.
    #[must_use]
    pub const fn interpretation(&self) -> &Interpretation {
        &self.interpretation
    }

    /// Consume the receipt and return raw interpretation data without copying it.
    #[must_use]
    pub fn into_interpretation(self) -> Interpretation {
        self.interpretation
    }
}
