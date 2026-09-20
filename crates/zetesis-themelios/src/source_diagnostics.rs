//! Original source diagnostics rendered through themelios's canonical human view.
//!
//! Parser refusals and located runtime errors retain their original source
//! identity, including nonzero IDs assigned after earlier roots or includes.
//! Do not remint that identity while constructing the rendering catalog or
//! reread possibly changed disk contents.

use std::fmt;

use themelios_base::{
    diagnostic::{Diagnostic, ToDiagnostic},
    line::LineIndex,
    source::{Source, SourceId, Sources},
    view,
};
use themelios_syntax::diagnostic::SyntaxError;

/// One original source selected by a typed diagnostic's retained identity.
/// The source owns its bytes; copying this context costs the source and name size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RetainedSource {
    pub name: String,
    pub source: Source,
}

impl RetainedSource {
    pub fn write(&self, output: &mut fmt::Formatter<'_>, diagnostic: &Diagnostic) -> fmt::Result {
        let context = Context {
            name: &self.name,
            source: &self.source,
            index: LineIndex::of(&self.source),
        };
        write_diagnostic(output, diagnostic, &context)
    }
}

struct Context<'a> {
    name: &'a str,
    source: &'a Source,
    index: LineIndex,
}

impl Sources for Context<'_> {
    fn name(&self, id: SourceId) -> Option<&str> {
        (id == self.source.id()).then_some(self.name)
    }

    fn text(&self, id: SourceId) -> Option<&str> {
        (id == self.source.id()).then_some(self.source.text())
    }

    fn line_index(&self, id: SourceId) -> Option<&LineIndex> {
        (id == self.source.id()).then_some(&self.index)
    }
}

/// Build one coherent index, then render each retained diagnostic in parser order.
/// The view preserves severity, code, primary/secondary labels, notes and help.
/// Work and temporary storage follow the source index and rendered diagnostic
/// size. Each write propagates formatter failure; no terminal policy lives here.
pub(crate) fn write(
    output: &mut fmt::Formatter<'_>,
    name: &str,
    source: &Source,
    errors: &[SyntaxError],
) -> fmt::Result {
    write_diagnostics(
        output,
        name,
        source,
        errors.iter().map(ToDiagnostic::to_diagnostic),
    )
}

/// Render typed diagnostics lazily against one original source and one index.
pub(crate) fn write_diagnostics(
    output: &mut fmt::Formatter<'_>,
    name: &str,
    source: &Source,
    diagnostics: impl Iterator<Item = Diagnostic>,
) -> fmt::Result {
    let context = Context {
        name,
        source,
        index: LineIndex::of(source),
    };
    for diagnostic in diagnostics {
        write_diagnostic(output, &diagnostic, &context)?;
    }
    Ok(())
}

pub(crate) fn write_diagnostic(
    output: &mut fmt::Formatter<'_>,
    diagnostic: &Diagnostic,
    sources: &impl Sources,
) -> fmt::Result {
    let rendered = view::human(diagnostic, sources);
    write!(output, "\n{}", rendered.trim_end_matches('\n'))
}

#[cfg(test)]
mod tests {
    use super::{Context, LineIndex, Source, SourceId};
    use themelios_base::source::check_sources_laws;

    #[test]
    fn rejected_source_catalog_preserves_nonzero_identity() {
        let source = Source::new(SourceId::new(7), "p(é).\r\n".into()).unwrap();
        let context = Context {
            name: "child.lp",
            index: LineIndex::of(&source),
            source: &source,
        };
        assert!(check_sources_laws(&context, &[SourceId::new(0), source.id()]).is_empty());
    }
}
