use super::super::super::{ConstructionLimits, ErrorKind, Limits, Resource, Statistics};
use super::{ModelRows, Work};
use zetesis_core::{AtomCatalog, Model, Sign, catalog::ReadError};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_test_support::programs::signed_numbered as atom;

fn work(cancellation: &Cancellation) -> Work<'_> {
    Work {
        limits: Limits::default(),
        construction: ConstructionLimits::default(),
        cancellation,
        statistics: Statistics::default(),
        location: None,
        local_bytes: 0,
    }
}

#[test]
fn predicate_windows_address_the_selected_model_order() {
    let catalog = AtomCatalog::new(vec![
        atom("p", Sign::Positive, &[2]),
        atom("q", Sign::Positive, &[0]),
        atom("p", Sign::Positive, &[1]),
        atom("p", Sign::Positive, &[3]),
        atom("a", Sign::Positive, &[0]),
    ])
    .unwrap();
    let model = Model::from_positions(&catalog, [0, 2, 4, 1]).unwrap();
    assert_eq!(model.positions(), &[4, 2, 0, 1]);
    let query = Model::new([atom("p", Sign::Positive, &[9])]).unwrap();
    let predicate = query.atoms().at(0).unwrap().predicate();
    assert!(matches!(
        catalog.read().declare_existing(predicate),
        Err(ReadError::ForeignCatalog)
    ));
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let rows = ModelRows::new(&model, &mut work).unwrap();
    let range = rows.predicate(predicate, &mut work).unwrap();
    assert_eq!(range, 1..3);
    let selected: Vec<_> = range.map(|row| rows.get(row)).collect();
    assert_eq!(
        selected,
        vec![
            catalog.atoms().at(2).unwrap(),
            catalog.atoms().at(0).unwrap()
        ]
    );
}

#[test]
fn predicate_windows_preserve_the_full_signed_signature() {
    let model = Model::new([
        atom("q", Sign::Positive, &[]),
        atom("p", Sign::Positive, &[1, 2]),
        atom("p", Sign::Negative, &[3]),
        atom("p", Sign::Positive, &[2]),
        atom("p", Sign::Negative, &[]),
        atom("p", Sign::Positive, &[]),
        atom("p", Sign::Positive, &[1]),
    ])
    .unwrap();
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let rows = ModelRows::new(&model, &mut work).unwrap();
    for (query, expected) in [
        (atom("p", Sign::Positive, &[]), 0..1),
        (atom("p", Sign::Negative, &[]), 1..2),
        (atom("p", Sign::Positive, &[0]), 2..4),
        (atom("p", Sign::Negative, &[0]), 4..5),
        (atom("p", Sign::Positive, &[0, 0]), 5..6),
        (atom("q", Sign::Positive, &[]), 6..7),
    ] {
        let query = Model::new([query]).unwrap();
        let predicate = query.atoms().at(0).unwrap().predicate();
        assert!(matches!(
            model.catalog().read().declare_existing(predicate),
            Err(ReadError::ForeignCatalog)
        ));
        assert_eq!(rows.predicate(predicate, &mut work).unwrap(), expected);
    }
}

#[test]
fn a_shared_predicate_does_not_scan_its_name() {
    let cancellation = Cancellation::default();
    let mut charges = Vec::new();
    for name in ["p".to_owned(), "p".repeat(4096)] {
        let model = Model::new([atom(&name, Sign::Positive, &[])]).unwrap();
        let predicate = model.atoms().at(0).unwrap().predicate();
        model.catalog().read().declare_existing(predicate).unwrap();
        let mut work = work(&cancellation);
        let rows = ModelRows::new(&model, &mut work).unwrap();
        assert_eq!(rows.predicate(predicate, &mut work).unwrap(), 0..1);
        charges.push(work.statistics.work);
    }
    assert_eq!(charges[0], charges[1]);
}

#[test]
fn foreign_predicates_are_resolved_by_content() {
    let model = Model::new([
        atom("p", Sign::Positive, &[1]),
        atom("p", Sign::Positive, &[2]),
        atom("q", Sign::Positive, &[3]),
    ])
    .unwrap();
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let rows = ModelRows::new(&model, &mut work).unwrap();
    for (name, expected) in [("p", 0..2), ("q", 2..3)] {
        // Each query's first predicate belongs to a fresh canonical authority.
        let query = Model::new([atom(name, Sign::Positive, &[0])]).unwrap();
        let predicate = query.atoms().at(0).unwrap().predicate();
        assert!(matches!(
            model.catalog().read().declare_existing(predicate),
            Err(ReadError::ForeignCatalog)
        ));
        assert_eq!(rows.predicate(predicate, &mut work).unwrap(), expected);
    }
}

#[test]
fn an_absent_predicate_has_an_empty_window() {
    let cancellation = Cancellation::default();
    let query = Model::new([atom("q", Sign::Positive, &[])]).unwrap();
    let predicate = query.atoms().at(0).unwrap().predicate();
    for (model, expected) in [
        (Model::new([]).unwrap(), 0..0),
        (Model::new([atom("p", Sign::Positive, &[])]).unwrap(), 1..1),
    ] {
        let mut work = work(&cancellation);
        let rows = ModelRows::new(&model, &mut work).unwrap();
        assert_eq!(rows.predicate(predicate, &mut work).unwrap(), expected);
    }
}

#[test]
fn every_work_cut_refuses_the_unfinished_window() {
    let model = Model::new([
        atom("prefix_p", Sign::Positive, &[1]),
        atom("prefix_p", Sign::Positive, &[2]),
        atom("prefix_q", Sign::Positive, &[3]),
    ])
    .unwrap();
    let query = Model::new([atom("prefix_p", Sign::Positive, &[0])]).unwrap();
    let predicate = query.atoms().at(0).unwrap().predicate();
    assert!(matches!(
        model.catalog().read().declare_existing(predicate),
        Err(ReadError::ForeignCatalog)
    ));
    let cancellation = Cancellation::default();
    let rows = ModelRows::new(&model, &mut work(&cancellation)).unwrap();
    let mut complete = work(&cancellation);
    assert_eq!(rows.predicate(predicate, &mut complete).unwrap(), 0..2);
    for maximum in 0..complete.statistics.work {
        let mut limited = work(&cancellation);
        limited.limits.max_work = maximum;
        let error = rows.predicate(predicate, &mut limited).unwrap_err();
        assert_eq!(
            error.kind(),
            &ErrorKind::Limit {
                resource: Resource::Work,
                limit: u128::from(maximum),
                observed: u128::from(maximum) + 1,
            }
        );
        assert_eq!(limited.statistics.work, maximum);
    }
}

#[test]
fn cancelled_predicate_lookup_returns_the_stop() {
    let model = Model::new([atom("p", Sign::Positive, &[])]).unwrap();
    let predicate = model.atoms().at(0).unwrap().predicate();
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let rows = ModelRows::new(&model, &mut work).unwrap();
    cancellation.cancel();
    let error = rows.predicate(predicate, &mut work).unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::Stopped(Stop::Cancelled));
}
