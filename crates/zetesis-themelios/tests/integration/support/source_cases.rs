//! Original sources and complete records decoded from campaign fixtures.
use serde_json::Value as Json;
use zetesis_clingo_support::{self as oracle, atoms, costs};
use zetesis_reference_support as reference;
use zetesis_test_support::records::Records;
use zetesis_themelios::FormulaLimits;

pub(crate) struct Case {
    pub name: String,
    pub source: String,
    pub records: Records,
}
pub(crate) fn cases(fixture: &str) -> Vec<Case> {
    fixture
        .lines()
        .map(|line| {
            let row: Json = serde_json::from_str(line).unwrap();
            Case {
                name: row["name"].as_str().unwrap().into(),
                source: row["source"].as_str().unwrap().into(),
                records: row["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|record| (atoms(&record[0]), costs(&record[1])))
                    .collect(),
            }
        })
        .collect()
}

/// Every case in `fixture` admits with the reduct subject of its program
/// without its objective statements: the same atoms, theory and formula
/// origins.
pub(crate) fn objectives_keep_the_original_reduct_subject(fixture: &str) {
    for case in cases(fixture) {
        let program = case
            .source
            .split("#minimize")
            .next()
            .unwrap()
            .split("#maximize")
            .next()
            .unwrap()
            .split(":~")
            .next()
            .unwrap();
        let original = reference::admit(program, &FormulaLimits::default()).unwrap();
        let observed = reference::admit(&case.source, &FormulaLimits::default()).unwrap();
        assert_eq!(original.atoms(), observed.atoms(), "{}", case.name);
        assert_eq!(
            (original.theory().nodes(), original.theory().operands()),
            (observed.theory().nodes(), observed.theory().operands()),
            "{}",
            case.name
        );
        assert_eq!(
            original.theory().roots(),
            observed.theory().roots(),
            "{}",
            case.name
        );
        assert_eq!(
            original.formula_origins(),
            observed.formula_origins(),
            "{}",
            case.name
        );
    }
}

/// Fresh clingo's complete scored records of every case's source are the ones
/// the case records.
pub(crate) fn clingo_matches_every_case(cases: &[Case]) {
    for case in cases {
        assert_eq!(
            oracle::records(&case.source),
            case.records,
            "{}: {}",
            case.name,
            case.source
        );
    }
}
