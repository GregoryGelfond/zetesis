//! A one-atom catalog, and a step budget that fails once spent, for the
//! canonical-form propositions.

use zetesis_core::{Atom, AtomCatalog, Predicate, Value};

/// Spends one step of `remaining`, failing once none is left.
pub(crate) fn before(remaining: &mut usize) -> Result<(), ()> {
    if *remaining == 0 {
        return Err(());
    }
    *remaining -= 1;
    Ok(())
}

/// The catalog of the one atom `p(value)`.
pub(crate) fn catalog(value: Value) -> AtomCatalog {
    AtomCatalog::new(vec![
        Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap(),
    ])
    .unwrap()
}

/// Store test template components without asserting any admission policy.
/// Head and body occurrences keep their supplied order in one component row.
pub(crate) fn template_catalog(
    templates: &[zetesis_core::Template],
) -> zetesis_core::TemplateCatalog {
    let mut builder = zetesis_core::TemplateCatalogBuilder::new(usize::MAX).unwrap();
    for template in templates {
        builder
            .append(
                [],
                template
                    .head()
                    .into_iter()
                    .chain(template.positive())
                    .chain(template.gate_true())
                    .chain(template.gate_false())
                    .map(zetesis_core::PatternRef::from),
                template.filters().iter().map(zetesis_core::FilterRef::from),
            )
            .unwrap();
    }
    builder.finish().unwrap()
}
