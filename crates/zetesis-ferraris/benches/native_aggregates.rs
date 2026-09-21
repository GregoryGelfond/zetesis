//! Aggregate-formula truth only; no source admission or stable-model solving.
//!
//! Separate populations measure preacquired-mask reduction, acquisition plus
//! reduction, and existing public lowered-formula calls. The last route invokes
//! `models` and `models_reduct`, whose API recomputes original truth for the reduct.
//! Group/lowering construction, worlds, masks and parity checks are outside timers.

use std::{hint::black_box, time::Duration};

use criterion::{BenchmarkId, Criterion, Throughput};
use zetesis_core::Value as Term;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison as Comparison, AggregateElement, AggregateExtremum,
    AggregateLimits, Interpretation, Node, Theory, ValueExtremumElement, append_aggregate,
    append_value_extremum, models, models_reduct,
    native_aggregate::{self as native, Bound, Function, Group, Guard, Tuple},
};

const WORLD_PAIRS: usize = 16;

struct Worlds {
    candidate: Interpretation,
    tested: Interpretation,
    lowered_candidate: Interpretation,
    lowered_tested: Interpretation,
    original: Vec<bool>,
    frozen: Vec<bool>,
    expected: (bool, bool),
}

struct Fixture {
    group: Group,
    lowered: Theory,
    worlds: Vec<Worlds>,
    cancellation: Cancellation,
}

fn original() -> Theory {
    Theory::new(
        2,
        vec![
            Node::False,
            Node::Implies(0, 0),
            Node::Atom(0),
            Node::Atom(1),
            Node::Implies(2, 0),
            Node::Implies(4, 0),
            Node::Implies(3, 0),
            Node::And(2, 3),
            Node::Or(2, 3),
            Node::Implies(2, 3),
            Node::Or(2, 4),
        ],
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn term(function: Function, index: usize) -> Term {
    match function {
        Function::Min | Function::Max => match index % 3 {
            0 => Term::Number(-2),
            1 => Term::Symbol("a".into()),
            _ => Term::String("a".into()),
        },
        _ => Term::Number(match index % 3 {
            0 => -2,
            1 => 3,
            _ => 0,
        }),
    }
}

fn retained_group(theory: &Theory, function: Function, count: usize, bound: i32) -> Group {
    let tuples = (0..count)
        .map(|index| Tuple {
            key: vec![
                term(function, index),
                Term::Number(i32::try_from(index).unwrap()),
            ],
            condition: match function {
                Function::Min | Function::Max => {
                    if index % 3 == 0 {
                        4
                    } else {
                        2
                    }
                }
                _ => 2 + index % 9,
            },
        })
        .collect();
    let guard = Guard {
        comparison: Comparison::Ge,
        bound: Bound::Integer(i128::from(bound)),
    };
    Group::new(
        theory,
        function,
        tuples,
        vec![guard],
        native::AdmissionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

fn fixture(function: Function, count: usize) -> Fixture {
    let theory = original();
    let threshold = match function {
        Function::Count | Function::SumPlus => i32::try_from(count / 2).unwrap(),
        _ => 0,
    };
    let group = retained_group(&theory, function, count, threshold);
    let lowered = lower(&group, threshold);
    let cancellation = Cancellation::default();
    let mut worlds = Vec::new();
    let mut acquisition_work = 0;
    let mut reduction_work = 0;
    for m in 0..4 {
        for j in 0..4 {
            let candidate = world(&theory, m);
            let tested = world(&theory, j);
            let lowered_candidate = world(&lowered, m);
            let lowered_tested = world(&lowered, j);
            let expected = (
                models(
                    &lowered,
                    &lowered_candidate,
                    zetesis_ferraris::Limits::default(),
                    &cancellation,
                )
                .unwrap(),
                models_reduct(
                    &lowered,
                    &lowered_candidate,
                    &lowered_tested,
                    zetesis_ferraris::Limits::default(),
                    &cancellation,
                )
                .unwrap(),
            );
            let acquired = group
                .eligibility(
                    &candidate,
                    Some(&tested),
                    native::EligibilityLimits::default(),
                    &cancellation,
                )
                .unwrap();
            let result = acquired
                .reduce(native::ReductionLimits::default(), &cancellation)
                .unwrap();
            assert_eq!(
                (result.original().holds(), result.reduct_truth().unwrap()),
                expected
            );
            acquisition_work += acquired.statistics().work;
            reduction_work += result.statistics().work;
            let original = acquired.original().to_vec();
            let frozen = acquired.frozen().unwrap().to_vec();
            worlds.push(Worlds {
                candidate,
                tested,
                lowered_candidate,
                lowered_tested,
                original,
                frozen,
                expected,
            });
        }
    }
    assert_eq!(worlds.len(), WORLD_PAIRS);
    let original_true = worlds.iter().filter(|world| world.expected.0).count();
    let reduct_true = worlds.iter().filter(|world| world.expected.1).count();
    assert!(original_true > 0 && original_true < WORLD_PAIRS);
    assert!(reduct_true > 0 && reduct_true < WORLD_PAIRS);
    eprintln!(
        "NATIVE_AGGREGATE function={function:?} tuples={count} pairs={WORLD_PAIRS} original_true={original_true} reduct_true={reduct_true} eligibility_nodes={} lowered_nodes={} admission_work={} admission_peak_bytes={} retained_group_bytes={} acquisition_work={acquisition_work} reduction_work={reduction_work}",
        theory.nodes().len(),
        lowered.nodes().len(),
        group.statistics().work,
        group.statistics().peak_bytes,
        group.statistics().resident_bytes
    );
    Fixture {
        group,
        lowered,
        worlds,
        cancellation,
    }
}

fn world(theory: &Theory, bits: usize) -> Interpretation {
    Interpretation::new(theory, (0..2).filter(|atom| bits & (1 << atom) != 0)).unwrap()
}

fn lower(group: &Group, bound: i32) -> Theory {
    let mut nodes = group.theory().nodes().to_vec();
    let root = match group.function() {
        Function::Min | Function::Max => {
            let elements: Vec<_> = group
                .tuples()
                .iter()
                .map(|tuple| ValueExtremumElement {
                    value: tuple.key[0].clone(),
                    condition: tuple.condition,
                })
                .collect();
            let kind = if group.function() == Function::Min {
                AggregateExtremum::Min
            } else {
                AggregateExtremum::Max
            };
            append_value_extremum(
                &mut nodes,
                &elements,
                kind,
                Comparison::Ge,
                &Term::Number(bound),
                AggregateLimits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .root()
        }
        _ => {
            let elements: Vec<_> = group
                .tuples()
                .iter()
                .map(|tuple| {
                    let Term::Number(value) = tuple.key[0] else {
                        panic!("numeric fixture contribution");
                    };
                    let weight = match group.function() {
                        Function::Count => 1,
                        Function::SumPlus => value.max(0),
                        _ => value,
                    };
                    AggregateElement {
                        weight,
                        condition: tuple.condition,
                    }
                })
                .collect();
            append_aggregate(
                &mut nodes,
                &elements,
                Comparison::Ge,
                i64::from(bound),
                AggregateLimits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .root()
        }
    };
    Theory::new(
        group.theory().atom_count(),
        nodes,
        vec![root],
        AdmissionLimits::default(),
    )
    .unwrap()
}

impl Fixture {
    fn masks(&self) -> [(bool, bool); WORLD_PAIRS] {
        std::array::from_fn(|index| {
            let world = &self.worlds[index];
            let result = self
                .group
                .reduce(
                    &world.original,
                    Some(&world.frozen),
                    native::ReductionLimits::default(),
                    &self.cancellation,
                )
                .unwrap();
            (result.original().holds(), result.reduct_truth().unwrap())
        })
    }

    fn acquire(&self) -> [(bool, bool); WORLD_PAIRS] {
        std::array::from_fn(|index| {
            let world = &self.worlds[index];
            let acquired = self
                .group
                .eligibility(
                    &world.candidate,
                    Some(&world.tested),
                    native::EligibilityLimits::default(),
                    &self.cancellation,
                )
                .unwrap();
            let result = acquired
                .reduce(native::ReductionLimits::default(), &self.cancellation)
                .unwrap();
            (result.original().holds(), result.reduct_truth().unwrap())
        })
    }

    fn lowered(&self) -> [(bool, bool); WORLD_PAIRS] {
        std::array::from_fn(|index| {
            let world = &self.worlds[index];
            (
                models(
                    &self.lowered,
                    &world.lowered_candidate,
                    zetesis_ferraris::Limits::default(),
                    &self.cancellation,
                )
                .unwrap(),
                models_reduct(
                    &self.lowered,
                    &world.lowered_candidate,
                    &world.lowered_tested,
                    zetesis_ferraris::Limits::default(),
                    &self.cancellation,
                )
                .unwrap(),
            )
        })
    }
}

fn native_aggregates(criterion: &mut Criterion) {
    let mut benchmark = criterion.benchmark_group("native_aggregates");
    benchmark.sample_size(20);
    benchmark.warm_up_time(Duration::from_secs(1));
    benchmark.measurement_time(Duration::from_secs(1));
    for function in [
        Function::Count,
        Function::Sum,
        Function::SumPlus,
        Function::Min,
        Function::Max,
    ] {
        let counts = if function == Function::Sum {
            [4, 8]
        } else {
            [8, 64]
        };
        for count in counts {
            let fixture = fixture(function, count);
            let expected: [(bool, bool); WORLD_PAIRS] =
                std::array::from_fn(|index| fixture.worlds[index].expected);
            assert_eq!(fixture.masks(), expected);
            assert_eq!(fixture.acquire(), expected);
            assert_eq!(fixture.lowered(), expected);
            let cells = u64::try_from(2 * count * fixture.worlds.len()).unwrap();
            benchmark.throughput(Throughput::Elements(cells));
            let label = format!("{function:?}-{count}");
            benchmark.bench_function(BenchmarkId::new("masks", &label), |bencher| {
                bencher.iter(|| black_box(fixture.masks()));
            });
            benchmark.bench_function(BenchmarkId::new("acquire_reduce", &label), |bencher| {
                bencher.iter(|| black_box(fixture.acquire()));
            });
            benchmark.bench_function(
                BenchmarkId::new("lowered_public_calls", &label),
                |bencher| bencher.iter(|| black_box(fixture.lowered())),
            );
        }
    }
    benchmark.finish();
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    native_aggregates(&mut criterion);
    criterion.final_summary();
}
