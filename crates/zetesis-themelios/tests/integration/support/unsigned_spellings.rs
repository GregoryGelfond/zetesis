//! Atoms spelled as clingo spells them, for programs without strong negation.

/// `atom` as clingo spells it, without a strong-negation sign.
pub fn atom_text<'a>(atom: impl Into<zetesis_core::catalog::AtomRef<'a>>) -> String {
    let atom = atom.into();
    let name = atom.predicate().name();
    if atom.values().is_empty() {
        return name.to_owned();
    }
    let values: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value.descriptor() {
            zetesis_core::ValueNodeRef::Number(number) => number.to_string(),
            zetesis_core::ValueNodeRef::Symbol(symbol) => symbol.to_owned(),
            zetesis_core::ValueNodeRef::String(value) => {
                serde_json::to_string(value).expect("quoted scalar string")
            }
            zetesis_core::ValueNodeRef::Infimum => "#inf".to_owned(),
            zetesis_core::ValueNodeRef::Supremum => "#sup".to_owned(),
            zetesis_core::ValueNodeRef::Function { .. }
            | zetesis_core::ValueNodeRef::Tuple { .. } => value.to_string(),
        })
        .collect();
    format!("{name}({})", values.join(","))
}
