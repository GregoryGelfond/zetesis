//! Both record formats retain compound arithmetic and attempted-publication order.

use super::{AtomTable, Counts, Error, Issue, Json, Limits, ModelRecord, Resource};

fn decode(
    table_form: bool,
    raw: &Json,
    limit: usize,
    counts: &mut Counts,
) -> Result<ModelRecord, Error> {
    let limits = Limits {
        report: crate::answers::Limits {
            max_symbols: limit,
            ..Default::default()
        },
        ..Default::default()
    };
    if table_form {
        super::record(raw, limits, counts, &mut AtomTable::default())
    } else {
        super::spelled_record(raw, limits, counts)
    }
}

#[test]
fn both_shown_additions_precede_the_symbol_ceiling() {
    // The shown payload would be invalid if decoding reached it. Both lengths
    // must enter the complete count before either the ceiling or decoding.
    let raw = serde_json::json!({
        "atoms": [],
        "full_model": [],
        "shown": {"atom_indices": [0], "terms": [null]},
        "costs": null,
    });
    for table_form in [false, true] {
        for prefix in [usize::MAX, usize::MAX - 1] {
            let mut counts = Counts {
                shown: prefix,
                ..Default::default()
            };
            assert!(matches!(
                decode(table_form, &raw, 0, &mut counts),
                Err(Error::Invalid {
                    issue: Issue::CountOverflow,
                    detail: "native shown occurrence count",
                })
            ));
            assert_eq!(counts.shown, prefix);
            assert_eq!(counts.atoms, 0);
            assert_eq!(counts.nodes, 0);
        }
    }
}

#[test]
fn a_shown_limit_refusal_keeps_the_complete_attempt() {
    let raw = serde_json::json!({
        "atoms": [],
        "full_model": [],
        "shown": {"atom_indices": [0], "terms": [null]},
        "costs": null,
    });
    for table_form in [false, true] {
        let mut counts = Counts {
            shown: 3,
            ..Default::default()
        };
        assert!(matches!(
            decode(table_form, &raw, 4, &mut counts),
            Err(Error::Limit {
                resource: Resource::Symbols,
                limit: 4,
                attempted: 5,
            })
        ));
        assert_eq!(counts.shown, 5);
        assert_eq!(counts.atoms, 0);
        assert_eq!(counts.nodes, 0);
    }
}

#[test]
fn empty_shown_payload_rechecks_the_inclusive_ceiling() {
    let raw = serde_json::json!({
        "atoms": [],
        "full_model": [],
        "shown": {"atom_indices": [], "terms": []},
        "costs": null,
    });
    for table_form in [false, true] {
        let mut counts = Counts {
            shown: 2,
            ..Default::default()
        };
        let record = decode(table_form, &raw, 2, &mut counts).unwrap();
        assert!(record.atoms.is_empty());
        assert!(record.shown_indices.is_empty());
        assert!(record.shown_terms.is_empty());
        assert!(record.costs.is_none());
        assert_eq!(counts.shown, 2);
        assert!(matches!(
            decode(table_form, &raw, 1, &mut counts),
            Err(Error::Limit {
                resource: Resource::Symbols,
                limit: 1,
                attempted: 2,
            })
        ));
        assert_eq!(counts.shown, 2);
    }
}
