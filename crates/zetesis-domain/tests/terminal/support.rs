use themelios_base::source::{Source, SourceId};
use themelios_program::program::Program;
use themelios_program::raise::raise;
use themelios_syntax::{dialect::Dialect, parse::parse};

pub fn source(text: &str) -> Program {
    let source = Source::new(SourceId::new(23), text.to_owned()).unwrap();
    let parsed = parse(&source, Dialect::Clingo);
    assert!(
        parsed.diagnostics().is_empty(),
        "{text}: {:?}",
        parsed.diagnostics()
    );
    let raised = raise(&parsed);
    assert!(
        raised.diagnostics().is_empty(),
        "{text}: {:?}",
        raised.diagnostics()
    );
    raised.program().clone()
}
