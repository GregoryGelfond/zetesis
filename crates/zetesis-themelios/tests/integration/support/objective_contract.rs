//! Exact scored-family contracts for a preserved source with appended objectives.

use serde_json::Value as Json;
use zetesis_clingo_support as oracle;
use zetesis_reference_support as reference;
use zetesis_test_support::records::Records;
use zetesis_themelios::FormulaLimits;

pub(crate) fn check(fixture: &str, source: &str) {
    let mut rows = fixture
        .lines()
        .map(|line| serde_json::from_str::<Json>(line).unwrap())
        .filter(|row| row["source"] == source);
    let row = rows
        .next()
        .unwrap_or_else(|| panic!("missing complete objective contract: {source}"));
    assert!(
        rows.next().is_none(),
        "duplicate objective contract: {source}"
    );
    let expected: Records = row["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| (oracle::atoms(&record[0]), oracle::costs(&record[1])))
        .collect();
    let input = reference::admit(source, &FormulaLimits::default()).unwrap();
    assert_eq!(input.source().expect("source input").text(), source);
    assert_eq!(reference::exhaustive(&input), expected, "{}", row["name"]);
    assert_eq!(
        serde_json::to_value(input.objectives().priorities()).unwrap(),
        row["priorities"]
    );
    // These curated sources append optimization statements to the unchanged
    // program. The objective may read its models but cannot alter its theory.
    let objective = [source.find("#minimize"), source.find("#maximize")]
        .into_iter()
        .flatten()
        .min()
        .expect("the preserved source has an appended objective");
    let original = reference::admit(&source[..objective], &FormulaLimits::default()).unwrap();
    assert_eq!(input.atoms(), original.atoms());
    assert_eq!(
        (input.theory().nodes(), input.theory().operands()),
        (original.theory().nodes(), original.theory().operands())
    );
    assert_eq!(input.theory().roots(), original.theory().roots());
    assert_eq!(input.formula_origins(), original.formula_origins());
}
