//! Canonical sources use the same relation-local equality coordinates and masks.

use super::*;
use zetesis_core::{AtomCatalog, Sign};

#[test]
fn canonical_relation_packing_preserves_occurrences() {
    let predicate = Predicate::with_sign("row", 2, Sign::Negative).unwrap();
    let source = [
        [Value::String("1".into()), Value::Number(2)],
        [Value::Symbol("1".into()), Value::Number(2)],
        [Value::String("1".into()), Value::Number(1)],
    ]
    .map(|values| Atom::new(predicate.clone(), values.into()).unwrap());
    let indices = [2, 0, 2, 1];
    let catalog = AtomCatalog::new(source.to_vec()).unwrap();
    let canonical = Relation::from_catalog_refs(
        catalog.atoms().at(0).unwrap().predicate(),
        catalog.atoms(),
        &indices,
        relation::Limits::default(),
    )
    .unwrap();
    let ingress =
        Relation::from_catalog(&predicate, &source, &indices, relation::Limits::default()).unwrap();
    assert_eq!(
        canonical.columns().collect::<Vec<_>>(),
        ingress.columns().collect::<Vec<_>>()
    );

    // The query term belongs to a separately admitted catalog. It is resolved
    // by typed equality into this Relation's dictionary, never by a raw ID.
    let foreign = AtomCatalog::new(vec![source[0].clone()]).unwrap();
    let term = foreign.atoms().at(0).unwrap().values().at(0).unwrap();
    let canonical_queries = [canonical
        .query(&[(0, term)], relation::Limits::default())
        .unwrap()];
    let ingress_queries = [ingress
        .query(
            &[(0, (&source[0].values()[0]).into())],
            relation::Limits::default(),
        )
        .unwrap()];
    let mut canonical_plan = plan(&canonical, &canonical_queries);
    let mut ingress_plan = plan(&ingress, &ingress_queries);
    let packed = canonical_plan
        .pack(&canonical_queries, &Cancellation::default(), u64::MAX)
        .unwrap();
    let reference = ingress_plan
        .pack(&ingress_queries, &Cancellation::default(), u64::MAX)
        .unwrap();
    assert_eq!(packed.records, reference.records);
    assert_eq!(packed.equalities, reference.equalities);

    // Synthetic receipts qualify the host decoder only. Physical tests exercise
    // the same canonical catalog route with an actual device separately.
    let input = records(&mut canonical_plan, &canonical_queries, &[vec![0b111]]);
    let masks = canonical_plan
        .decode(
            &canonical,
            &canonical_queries,
            &input,
            output(&canonical_plan),
            &Cancellation::default(),
        )
        .unwrap();
    let selected = masks.selection(0, relation::Limits::default()).unwrap();
    assert_eq!(selected.positions(), [0, 1, 2]);
    let original: Vec<_> = (0..selected.positions().len())
        .map(|index| selected.row(index).unwrap().source_index())
        .collect();
    assert_eq!(original, [2, 0, 2]);
    for (index, &source_index) in original.iter().enumerate() {
        assert_eq!(selected.row(index).unwrap().atom(), source[source_index]);
    }
}
