//! Original canonical input and optional source bytes retained by every formula owner.

use std::{fmt, sync::Arc};

use themelios_base::source::Source;
use themelios_program::program::Program;

use crate::{FormulaFailure, FormulaWarning, SourceBundle};

#[derive(Debug)]
enum Sources {
    Single(Source),
    Bundle(SourceBundle),
    None,
}

/// Program identity precedes normalization; source text is independent evidence.
#[derive(Debug)]
pub(crate) struct Owner {
    program: Arc<Program>,
    sources: Sources,
}

impl Owner {
    pub(crate) fn single(program: Arc<Program>, source: Source) -> Self {
        Self {
            program,
            sources: Sources::Single(source),
        }
    }

    pub(crate) fn bundle(program: Arc<Program>, bundle: SourceBundle) -> Self {
        Self {
            program,
            sources: Sources::Bundle(bundle),
        }
    }

    pub(crate) fn logical(program: Arc<Program>) -> Self {
        Self {
            program,
            sources: Sources::None,
        }
    }

    pub(crate) fn program(&self) -> &Program {
        &self.program
    }

    pub(crate) fn source(&self) -> Option<&Source> {
        match &self.sources {
            Sources::Single(source) => Some(source),
            Sources::Bundle(_) | Sources::None => None,
        }
    }

    pub(crate) fn source_bundle(&self) -> Option<&SourceBundle> {
        match &self.sources {
            Sources::Bundle(bundle) => Some(bundle),
            Sources::Single(_) | Sources::None => None,
        }
    }

    /// Bundle receipts are constructed only from `Owner::bundle`; single and
    /// logical receipts expose the optional `source_bundle` view instead.
    pub(crate) fn required_bundle(&self) -> &SourceBundle {
        let Sources::Bundle(bundle) = &self.sources else {
            unreachable!("a prepared source bundle retains its original bundle")
        };
        bundle
    }

    pub(crate) fn into_bundle(self) -> SourceBundle {
        let Sources::Bundle(bundle) = self.sources else {
            unreachable!("a prepared source bundle retains its original bundle")
        };
        bundle
    }

    pub(crate) fn retain_failure(&self, error: FormulaFailure) -> FormulaFailure {
        if matches!(self.sources, Sources::None) {
            FormulaFailure::Program {
                program: Arc::clone(&self.program),
                error: Box::new(error),
            }
        } else {
            error
        }
    }

    pub(crate) fn warning_view<'a>(
        &'a self,
        warnings: &'a [FormulaWarning],
    ) -> impl fmt::Display + 'a {
        WarningView {
            owner: self,
            warnings,
        }
    }
}

struct WarningView<'a> {
    owner: &'a Owner,
    warnings: &'a [FormulaWarning],
}

impl fmt::Display for WarningView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.owner.sources {
            Sources::Single(source) => {
                crate::formula_warning::source_view(self.warnings, source).fmt(f)
            }
            Sources::Bundle(bundle) => {
                crate::formula_warning::bundle_view(self.warnings, bundle).fmt(f)
            }
            Sources::None => crate::formula_warning::program_view(self.warnings).fmt(f),
        }
    }
}
