//! Indexed readings denote the materialized cube, including absent typed keys.

use super::*;
use crate::oracle::Gates;
use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, GateIndex, Predicate, Sign, Template, Term, Value,
};

struct Fixture {
    program: Program,
    held: CarrierSet,
    root: Vec<Arc<GateAtom>>,
    queries: Vec<AtomPattern>,
}

fn pattern(name: &str, sign: Sign, values: &[Value]) -> AtomPattern {
    AtomPattern::new(
        Predicate::with_sign(name, values.len(), sign).unwrap(),
        values.iter().cloned().map(Term::Constant).collect(),
    )
    .unwrap()
}

fn owned(pattern: &AtomPattern) -> Atom {
    pattern
        .key(&[] as &[Value])
        .unwrap()
        .to_atom(zetesis_core::ValueLimits::default())
        .unwrap()
}

fn fixture() -> Fixture {
    let held = pattern("held", Sign::Positive, &[]);
    let mut queries = vec![
        pattern("p", Sign::Positive, &[Value::Number(0)]),
        pattern("p", Sign::Positive, &[Value::Symbol("0".into())]),
        pattern("p", Sign::Positive, &[Value::String("0".into())]),
        pattern("q", Sign::Negative, &[]),
        pattern("q", Sign::Positive, &[]),
    ];
    let mut templates: Vec<_> = queries
        .iter()
        .chain([&held])
        .map(|atom| {
            Template::new(
                Some(atom.clone()),
                vec![],
                vec![atom.clone()],
                vec![],
                vec![],
            )
        })
        .collect();
    templates.push(Template::new(
        Some(held.clone()),
        vec![],
        vec![],
        vec![],
        vec![],
    ));
    templates.push(Template::new(
        Some(pattern("domain", Sign::Positive, &[Value::Number(1)])),
        vec![],
        vec![],
        vec![],
        vec![],
    ));
    let program = Program::new(templates, AdmissionLimits::default()).unwrap();
    let index = GateIndex::new(&program).unwrap();
    let mut root: Vec<_> = queries
        .iter()
        .map(|atom| Arc::new(index.locate(&owned(atom)).unwrap()))
        .collect();
    root.sort_unstable_by(|left, right| left.atom().cmp(&right.atom()));
    queries.push(held.clone());
    // This belongs to the symbolic carrier but not the completed supported root.
    queries.push(pattern("p", Sign::Positive, &[Value::Number(1)]));
    let held = [program.locate_atom(&owned(&held), true).unwrap().unwrap()]
        .into_iter()
        .collect();
    Fixture {
        program,
        held,
        root,
        queries,
    }
}

fn regions(atoms: usize) -> impl Iterator<Item = Region> {
    (0..3_usize.pow(u32::try_from(atoms).unwrap())).map(move |mut digits| {
        let mut region = Region::all_open(atoms);
        for at in 0..atoms {
            match digits % 3 {
                1 => {
                    region.hold(at);
                }
                2 => {
                    region.cut(at);
                }
                _ => {}
            }
            digits /= 3;
        }
        region
    })
}

fn materialize(fixture: &Fixture, region: &Region) -> Cube {
    let mut cube = Cube {
        must: fixture.held.clone(),
        may: Some(fixture.held.clone()),
    };
    for (at, gate) in fixture.root.iter().enumerate() {
        if region.is_held(at) {
            cube.must.insert(gate.carrier());
        }
        if !region.is_cut(at) {
            cube.may.as_mut().unwrap().insert(gate.carrier());
        }
    }
    cube
}

#[test]
fn region_gate_readings_equal_owned_bounds() {
    let fixture = fixture();
    let cancellation = crate::Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    for region in regions(fixture.root.len()) {
        let cube = materialize(&fixture, &region);
        let indexed = RegionBounds::new(&fixture.held, &fixture.root, &region);
        let borrowed = Bounds::Region(&indexed);
        for query in &fixture.queries {
            let key = query.key(&[] as &[Value]).unwrap();
            for required in [false, true] {
                assert_eq!(
                    Gates::Definite(borrowed)
                        .holds(&key, required, &mut work)
                        .unwrap(),
                    Gates::Definite((&cube).into())
                        .holds(&key, required, &mut work)
                        .unwrap(),
                    "definite {key:?}, required={required}, {region:?}",
                );
                assert_eq!(
                    Gates::Possible(borrowed)
                        .holds(&key, required, &mut work)
                        .unwrap(),
                    Gates::Possible((&cube).into())
                        .holds(&key, required, &mut work)
                        .unwrap(),
                    "possible {key:?}, required={required}, {region:?}",
                );
            }
        }
    }
}

#[test]
fn region_admission_equals_owned_bounds() {
    let mut fixture = fixture();
    // Exercise empty and nonempty fixed-held sets independently of decisions.
    for with_held in [true, false] {
        if !with_held {
            fixture.held.clear();
        }
        for region in regions(fixture.root.len()) {
            let cube = materialize(&fixture, &region);
            let indexed = RegionBounds::new(&fixture.held, &fixture.root, &region);
            let borrowed = Bounds::Region(&indexed);
            for required in [false, true] {
                let atom = fixture.queries[0].clone();
                let (positive, negative) = if required {
                    (vec![atom], vec![])
                } else {
                    (vec![], vec![atom])
                };
                let template = Template::new(None, vec![], positive, negative, vec![]);
                assert_eq!(
                    Gates::Definite(borrowed).admits((&template).into()),
                    Gates::Definite((&cube).into()).admits((&template).into()),
                    "required={required}, with_held={with_held}, {region:?}",
                );
            }
        }
    }
}

#[test]
fn a_lower_gate_outside_the_root_is_a_conflict() {
    let fixture = fixture();
    let region = Region::all_open(fixture.root.len());
    let unexpected = owned(fixture.queries.last().unwrap());
    let lower = Model::new([unexpected.clone()]).unwrap();
    let held = owned(
        fixture
            .queries
            .iter()
            .find(|pattern| pattern.predicate().name() == "held")
            .unwrap(),
    );
    let upper = Model::new([held, unexpected]).unwrap();
    let bounds = RegionBounds::new(&fixture.held, &fixture.root, &region);
    let cancellation = crate::Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    assert!(
        bounds
            .conflicts(&fixture.program, &lower, &upper, &mut work)
            .unwrap()
    );
}

#[test]
fn an_old_fixed_hold_missing_from_upper_is_a_conflict() {
    let fixture = fixture();
    let region = Region::all_open(fixture.root.len());
    let bounds = RegionBounds::new(&fixture.held, &fixture.root, &region);
    let cancellation = crate::Cancellation::default();
    let mut work = Work::source(&cancellation, u64::MAX);
    assert!(
        bounds
            .conflicts(
                &fixture.program,
                &Model::default(),
                &Model::default(),
                &mut work
            )
            .unwrap()
    );
}
