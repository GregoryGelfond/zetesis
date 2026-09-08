//! Original parser diagnostics rendered through themelios's canonical human view.
//!
//! A rejected input has not entered an admitted program. Its retained source
//! still owns the identity used by every parser label, including nonzero IDs
//! assigned after earlier roots or includes. Do not remint that identity while
//! constructing the rendering catalog or reread possibly changed disk contents.

use std::fmt;

use themelios_base::{
    diagnostic::ToDiagnostic,
    line::LineIndex,
    source::{Source, SourceId, Sources},
    view,
};
use themelios_syntax::diagnostic::SyntaxError;

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
    let context = Context {
        name,
        source,
        index: LineIndex::of(source),
    };
    for error in errors {
        let rendered = view::human(&error.to_diagnostic(), &context);
        write!(output, "\n{}", rendered.trim_end_matches('\n'))?;
    }
    Ok(())
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
