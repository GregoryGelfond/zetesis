//! Typed extrema preparation: repeated scalar witnesses versus a shared family.
//!
//! The scalar route emits Boolean witnesses; the family uses one native row. Timed
//! preparation excludes the input catalog, borrowed descriptors, prefix clone,
//! final theory admission and output destruction. Separate fixed-world timings
//! evaluate every guard in reused workspace, without grounding or search.
//! Four fixed interpretations and all sixteen M/J pairs check each guard against
//! the scalar route outside timing. This is bounded parity, not exhaustive ASP.

use std::{cell::OnceCell, hint::black_box, time::Duration};

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use zetesis_core::{
    Atom, AtomCatalog, Predicate, Sign, Value, ValueLimits, ValueNode, catalog::TermRef,
};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison, AggregateExtremum, AggregateFamilyBuild,
    AggregateFamilyLimits, AggregateLimits, EvaluationLimits, EvaluationWorkspace, FormulaNodes,
    FormulaParts, FrozenReduct, Interpretation, Limits, Node, Theory, ValueExtremumElement,
    ValueExtremumFamilySpace, ValueExtremumGuard, append_value_extremum_family_refs,
    append_value_extremum_refs,
};

const WORLDS: usize = 4;

#[derive(Clone, Copy)]
enum Terms {
    Numeric,
    Mixed,
}

impl Terms {
    fn label(self) -> &'static str {
        match self {
            Self::Numeric => "numeric",
            Self::Mixed => "mixed",
        }
    }

    fn value(self, index: usize) -> Value {
        let number = i32::try_from(index).unwrap() - 32;
        match (self, index % 4) {
            (Self::Numeric, _) | (Self::Mixed, 0) => Value::Number(number),
            (Self::Mixed, 1) => Value::Symbol(format!("key_{index:04}")),
            (Self::Mixed, 2) => Value::String(format!("shared_prefix_{index:04}")),
            (Self::Mixed, _) => {
                let mut nodes = vec![
                    ValueNode::Function {
                        name: "f".into(),
                        sign: Sign::Positive,
                        arity: 1,
                    };
                    6
                ];
                nodes.push(ValueNode::Number(number));
                Value::from_nodes(nodes, ValueLimits::default()).unwrap()
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Preparation {
    Scalar,
    Shared,
}

impl Preparation {
    fn label(self) -> &'static str {
        match self {
            Self::Scalar => "scalar_witnesses",
            // Earlier Boolean-family records used `shared_witness_family`.
            Self::Shared => "native_extremum_family",
        }
    }
}

struct Input {
    catalog: AtomCatalog,
    count: usize,
    guards: usize,
}

struct Borrowed<'a> {
    elements: Vec<ValueExtremumElement<TermRef<'a>>>,
    guards: Vec<ValueExtremumGuard<TermRef<'a>>>,
}

impl Input {
    fn new(terms: Terms, count: usize, guards: usize) -> Self {
        let mut values: Vec<_> = (0..count).map(|index| terms.value(index)).collect();
        for guard in 0..guards {
            let index = if guards == 1 {
                count / 2
            } else {
                count - 1 - guard * count / guards
            };
            values.push(values[index].clone());
        }
        let predicate = Predicate::new("term", 2).unwrap();
        let atoms = values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                Atom::new(
                    predicate.clone(),
                    vec![value, Value::Number(i32::try_from(index).unwrap())],
                )
                .unwrap()
            })
            .collect();
        Self {
            catalog: AtomCatalog::new(atoms).unwrap(),
            count,
            guards,
        }
    }

    fn borrow(&self) -> Borrowed<'_> {
        let term = |index| {
            self.catalog
                .atoms()
                .at(index)
                .unwrap()
                .values()
                .at(0)
                .unwrap()
        };
        Borrowed {
            elements: (0..self.count)
                .map(|condition| ValueExtremumElement {
                    value: term(condition),
                    condition,
                })
                .collect(),
            // Equality families represent the extrema-assignment consumer.
            guards: (0..self.guards)
                .map(|index| ValueExtremumGuard {
                    comparison: AggregateComparison::Eq,
                    bound: term(self.count + index),
                })
                .collect(),
        }
    }

    fn prefix(&self) -> FormulaParts {
        FormulaParts::new((0..self.count).map(Node::atom).collect(), vec![]).unwrap()
    }
}

enum Roots {
    Scalar(Vec<usize>),
    Shared(AggregateFamilyBuild),
}

impl Roots {
    fn as_slice(&self) -> &[usize] {
        match self {
            Self::Scalar(roots) => roots,
            Self::Shared(build) => build.roots(),
        }
    }

    fn retained_bytes(&self) -> usize {
        match self {
            Self::Scalar(roots) => size_of::<Vec<usize>>() + roots.capacity() * size_of::<usize>(),
            Self::Shared(build) => build.root_storage_bytes(),
        }
    }
}

struct Prepared {
    nodes: FormulaNodes,
    roots: Roots,
    work: u64,
    states: usize,
    auxiliary_peak_bytes: Option<u128>,
}

fn prepare(
    prefix: FormulaParts,
    input: &Borrowed<'_>,
    kind: AggregateExtremum,
    preparation: Preparation,
    control: &Cancellation,
) -> Prepared {
    let mut nodes = FormulaNodes::new(prefix);
    let (roots, work, states, auxiliary_peak_bytes) = match preparation {
        Preparation::Scalar => {
            let mut roots = Vec::with_capacity(input.guards.len());
            let mut work = 0;
            let mut states = 0;
            for guard in &input.guards {
                let built = append_value_extremum_refs(
                    &mut nodes,
                    input.elements.iter().cloned(),
                    kind,
                    guard.comparison,
                    guard.bound,
                    AggregateLimits::default(),
                    control,
                )
                .unwrap();
                roots.push(built.root());
                work += built.statistics().work;
                states = states.max(built.statistics().states);
            }
            // Scalar compilation does not report its auxiliary byte peak.
            (Roots::Scalar(roots), work, states, None)
        }
        Preparation::Shared => {
            let attempt = append_value_extremum_family_refs(
                &mut nodes,
                &input.elements,
                kind,
                &input.guards,
                AggregateFamilyLimits::default(),
                control,
            );
            let built = attempt.result.unwrap();
            let statistics = built.statistics();
            (
                Roots::Shared(built),
                statistics.work,
                statistics.states,
                Some(attempt.auxiliary_peak_bytes),
            )
        }
    };
    Prepared {
        nodes,
        roots,
        work,
        states,
        auxiliary_peak_bytes,
    }
}

struct Compiled {
    theory: Theory,
    roots: Vec<usize>,
}

impl Prepared {
    fn finish(self, count: usize, label: &str, preparation: Preparation) -> Compiled {
        eprintln!(
            "EXTREMUM_FAMILY fixture={label} preparation={} preparation_work={} temporary_states={} auxiliary_peak_bytes={:?} retained_nodes={} retained_occurrences={} formula_capacity_bytes={} root_capacity_bytes={}",
            preparation.label(),
            self.work,
            self.states,
            self.auxiliary_peak_bytes,
            self.nodes.parts().nodes().len(),
            self.nodes.parts().occurrences(),
            self.nodes.parts().retained_bytes(),
            self.roots.retained_bytes(),
        );
        let roots = self.roots.as_slice().to_vec();
        let theory = Theory::new(
            count,
            self.nodes.into_parts(),
            vec![],
            AdmissionLimits::default(),
        )
        .unwrap();
        Compiled { theory, roots }
    }
}

fn pattern(world: usize, atom: usize) -> bool {
    match world {
        0 => false,
        1 => true,
        2 => atom.is_multiple_of(2),
        _ => !atom.is_multiple_of(3),
    }
}

fn worlds(theory: &Theory) -> [Interpretation; WORLDS] {
    std::array::from_fn(|world| {
        Interpretation::new(
            theory,
            (0..theory.atom_count()).filter(|&atom| pattern(world, atom)),
        )
        .unwrap()
    })
}

fn parity(scalar: &Compiled, shared: &Compiled, control: &Cancellation) {
    assert_eq!(scalar.roots.len(), shared.roots.len());
    for (&left, &right) in scalar.roots.iter().zip(&shared.roots) {
        let single = |compiled: &Compiled, root| {
            Theory::new(
                compiled.theory.atom_count(),
                compiled.theory.parts().try_clone().unwrap(),
                vec![root],
                AdmissionLimits::default(),
            )
            .unwrap()
        };
        let left = single(scalar, left);
        let right = single(shared, right);
        let left_worlds = worlds(&left);
        let right_worlds = worlds(&right);
        for (left_candidate, right_candidate) in left_worlds.iter().zip(&right_worlds) {
            for (left_tested, right_tested) in left_worlds.iter().zip(&right_worlds) {
                let evaluate = |candidate: &Interpretation, tested: &Interpretation| {
                    let mut workspace = EvaluationWorkspace::default();
                    let original = workspace
                        .evaluate(candidate, EvaluationLimits::default(), control)
                        .result
                        .unwrap()
                        .is_model();
                    let frozen = FrozenReduct::new(candidate, Limits::default(), control)
                        .unwrap()
                        .is_satisfied_by(tested, Limits::default(), control)
                        .unwrap();
                    (original, frozen)
                };
                assert_eq!(
                    evaluate(left_candidate, left_tested),
                    evaluate(right_candidate, right_tested)
                );
            }
        }
    }
}

fn original_batch(
    compiled: &Compiled,
    workspace: &mut EvaluationWorkspace,
    worlds: &[Interpretation; WORLDS],
    control: &Cancellation,
) -> [(usize, u64); WORLDS] {
    std::array::from_fn(|index| {
        let attempt = workspace.evaluate(&worlds[index], EvaluationLimits::default(), control);
        let evaluation = attempt.result.unwrap();
        let checksum = compiled
            .roots
            .iter()
            .enumerate()
            .fold(0, |sum, (guard, &root)| {
                sum ^ if evaluation.node_truth(root).unwrap() {
                    guard + 1
                } else {
                    0
                }
            });
        (checksum, attempt.work)
    })
}

struct Fixture {
    input: Input,
    prefix: FormulaParts,
    scalar: Compiled,
    shared: Compiled,
}

impl Fixture {
    fn new(
        terms: Terms,
        count: usize,
        guards: usize,
        kind: AggregateExtremum,
        label: &str,
    ) -> Self {
        let control = Cancellation::default();
        let input = Input::new(terms, count, guards);
        let borrowed = input.borrow();
        let scalar = prepare(
            input.prefix(),
            &borrowed,
            kind,
            Preparation::Scalar,
            &control,
        )
        .finish(count, label, Preparation::Scalar);
        let shared = prepare(
            input.prefix(),
            &borrowed,
            kind,
            Preparation::Shared,
            &control,
        )
        .finish(count, label, Preparation::Shared);
        assert_eq!(shared.theory.parts().rows().len(), 1);
        assert_eq!(shared.theory.parts().extremum_cuts().len(), 2 * guards);
        parity(&scalar, &shared, &control);
        let space = ValueExtremumFamilySpace::new(count, guards).unwrap();
        eprintln!(
            "EXTREMUM_FAMILY_SPACE fixture={label} requested_family_states={} requested_family_auxiliary_bytes={} parity_worlds={WORLDS} parity_pairs={} guards={guards}",
            space.states(),
            space.storage_bytes(),
            WORLDS * WORLDS,
        );
        let prefix = input.prefix();
        Self {
            input,
            prefix,
            scalar,
            shared,
        }
    }

    fn compiled(&self, preparation: Preparation) -> &Compiled {
        match preparation {
            Preparation::Scalar => &self.scalar,
            Preparation::Shared => &self.shared,
        }
    }
}

pub(super) fn bench(criterion: &mut Criterion) {
    let mut benchmark = criterion.benchmark_group("typed_extremum_families");
    benchmark.sample_size(10);
    benchmark.warm_up_time(Duration::from_millis(250));
    benchmark.measurement_time(Duration::from_millis(500));
    let control = Cancellation::default();
    for (kind, kind_label) in [
        (AggregateExtremum::Min, "min"),
        (AggregateExtremum::Max, "max"),
    ] {
        for terms in [Terms::Numeric, Terms::Mixed] {
            for count in [8, 64] {
                for guards in [1, 4, count] {
                    let label = format!("{kind_label}-{}-E{count}-G{guards}", terms.label());
                    // Criterion invokes these closures only for matching IDs.
                    let fixture = OnceCell::new();
                    for preparation in [Preparation::Scalar, Preparation::Shared] {
                        let id = format!("{label}/{}", preparation.label());
                        benchmark.throughput(Throughput::Elements(u64::try_from(guards).unwrap()));
                        benchmark.bench_function(BenchmarkId::new("prepare", &id), |bencher| {
                            let fixture = fixture
                                .get_or_init(|| Fixture::new(terms, count, guards, kind, &label));
                            let input = fixture.input.borrow();
                            bencher.iter_batched(
                                || fixture.prefix.try_clone().unwrap(),
                                |prefix| {
                                    black_box(prepare(
                                        prefix,
                                        black_box(&input),
                                        kind,
                                        preparation,
                                        &control,
                                    ))
                                },
                                BatchSize::LargeInput,
                            );
                        });
                        benchmark.throughput(Throughput::Elements(
                            u64::try_from(WORLDS * guards).unwrap(),
                        ));
                        benchmark.bench_function(BenchmarkId::new("original_all_guards", &id), |bencher| {
                            let fixture = fixture.get_or_init(|| Fixture::new(terms, count, guards, kind, &label));
                            let compiled = fixture.compiled(preparation);
                            let worlds = worlds(&compiled.theory);
                            let mut workspace = EvaluationWorkspace::default();
                            workspace.reserve(&compiled.theory, EvaluationLimits::default().max_bytes, &control).unwrap();
                            let checked = original_batch(compiled, &mut workspace, &worlds, &control);
                            eprintln!(
                                "EXTREMUM_FAMILY_EVALUATION fixture={id} original_batch_work={} workspace_capacity_bytes={}",
                                checked.iter().map(|(_, work)| work).sum::<u64>(), workspace.retained_bytes(),
                            );
                            bencher.iter(|| black_box(original_batch(compiled, &mut workspace, black_box(&worlds), &control)));
                        });
                    }
                }
            }
        }
    }
    benchmark.finish();
}
