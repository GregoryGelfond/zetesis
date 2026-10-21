//! Shared device composition without changing either primitive's semantic subject.

use crate::support::physical;

use std::time::{Duration, Instant};
use zetesis_backend::{BatchComputeTime, ComputeTimingStatus, GpuApi};
use zetesis_core::{
    Atom, Predicate, Value,
    relation::{Limits, Relation},
};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory, TightPlan, TightPlanLimits};
use zetesis_wgpu::{
    FormulaCheck, FormulaLimits, FormulaVerdict, GpuContext, GpuError, GpuFormulaOracle,
    GpuOptions, GpuRelationExecutor, GpuSelection, GpuTightOracle, RelationGpuLimits,
    TightGpuError, TightGpuLimits,
};

fn qualify_timing(context: &GpuContext) -> bool {
    println!(
        "optional features: advertised={:?}; enabled={:?}",
        context.info().features(),
        context.features()
    );
    assert!(context.info().features().contains(context.features()));
    let timing_enabled = context
        .info()
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY);
    assert_eq!(
        context.compute_timing(),
        if timing_enabled {
            ComputeTimingStatus::Enabled
        } else {
            ComputeTimingStatus::Unsupported
        }
    );
    assert_eq!(
        context.features().contains(wgpu::Features::TIMESTAMP_QUERY),
        timing_enabled
    );
    timing_enabled
}

fn check_timing_sample(enabled: bool, report: Option<BatchComputeTime>) {
    // Enabling queries does not guarantee that the adapter resolves an interval.
    if let Some(BatchComputeTime::Measured(elapsed)) = report {
        assert!(enabled);
        assert!(!elapsed.is_zero());
    }
}

fn compose(backend: GpuApi) {
    let context =
        GpuContext::new_selected_with_timing(GpuOptions::default(), GpuSelection { api: backend })
            .unwrap();
    physical::verify(backend, context.info());
    let timing_enabled = qualify_timing(&context);
    let mut relation_executor = GpuRelationExecutor::from_context(&context).unwrap();
    let mut formula = GpuFormulaOracle::from_context(&context.clone()).unwrap();
    assert!(context.same_instance(relation_executor.context()));
    assert!(context.same_instance(formula.context()));

    let predicate = Predicate::new("value", 1).unwrap();
    let atoms = [
        Atom::new(predicate.clone(), vec![Value::Number(7)]).unwrap(),
        Atom::new(predicate.clone(), vec![Value::String("7".into())]).unwrap(),
    ];
    let indices = [1, 0, 0];
    let relation = Relation::from_catalog(&predicate, &atoms, &indices, Limits::default()).unwrap();
    let value = Value::Number(7);
    let queries = [relation
        .query(&[(0, (&value).into())], Limits::default())
        .unwrap()];
    let mut prepared = relation_executor
        .prepare(
            &relation,
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    // These are two different subjects on the same device. Equal bit widths are
    // not used to reinterpret row masks as candidate or aggregate membership.
    let theory = Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(vec![Node::atom(0)], Vec::new()).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
    let candidates = [
        Interpretation::new(&theory, []).unwrap(),
        Interpretation::new(&theory, [0]).unwrap(),
    ];
    for repetition in 0..2 {
        let checks = formula
            .propagate_batch(&theory, &candidates, FormulaLimits::default())
            .unwrap();
        assert_eq!(checks[0].verdict(), FormulaVerdict::NotModel);
        assert_eq!(checks[1].verdict(), FormulaVerdict::NoProperSubset);
        check_timing_sample(timing_enabled, formula.reported_compute_time());
        let stats = formula.last_batch_stats().unwrap();
        assert_eq!(stats.theory_uploaded, repetition == 0);
        assert_eq!(stats.transport_allocated, repetition == 0);
        if repetition == 0 {
            compare_timing_accounting(backend, &theory, &candidates, &checks, &formula);
        }
        let masks = prepared
            .filter(
                &queries,
                RelationGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert!(relation.same_owner(masks.relation()));
        let selected = masks.selection(0, Limits::default()).unwrap();
        assert_eq!(selected.positions(), [1, 2]);
        assert_eq!(selected.row(0).unwrap().source_index(), 0);
        assert_eq!(selected.row(1).unwrap().source_index(), 0);
        assert_eq!(prepared.activity().submissions, 1);
        assert_eq!(prepared.activity().completed_queries, 1);
    }
    // Dropping the original handle does not remove the device from its clients.
    drop(context);
    let checks = formula
        .propagate_batch(&theory, &candidates, FormulaLimits::default())
        .unwrap();
    assert_eq!(checks[1].verdict(), FormulaVerdict::NoProperSubset);
    assert!(!formula.last_batch_stats().unwrap().theory_uploaded);
    check_timing_sample(timing_enabled, formula.reported_compute_time());
    assert!(
        formula
            .propagate_batch(&theory, &[], FormulaLimits::default())
            .unwrap()
            .is_empty()
    );
    assert_eq!(formula.reported_compute_time(), None);
}

fn compare_timing_accounting(
    backend: GpuApi,
    theory: &Theory,
    candidates: &[Interpretation],
    checks: &[FormulaCheck],
    formula: &GpuFormulaOracle,
) {
    let mut untimed =
        GpuFormulaOracle::new_selected(GpuOptions::default(), GpuSelection { api: backend })
            .unwrap();
    let untimed_checks = untimed
        .propagate_batch(theory, candidates, FormulaLimits::default())
        .unwrap();
    assert_eq!(
        untimed.context().compute_timing(),
        ComputeTimingStatus::NotRequested
    );
    assert_eq!(untimed.reported_compute_time(), None);
    assert!(!untimed.compute_time_unresolved());
    let untimed_cold = untimed.last_batch_stats().unwrap();
    // The oracle's four query slots, two resolve buffers and flush readback, and
    // the transport's 16-byte readback tail: 96 logical bytes, excluding any
    // driver-private query allocation. Only the tail belongs to the transport.
    let (accounted, transport) =
        if formula.context().compute_timing() == ComputeTimingStatus::Enabled {
            (96, 16)
        } else {
            (0, 0)
        };

    let stats = formula.last_batch_stats().unwrap();
    assert_eq!(
        stats.resident_transport_bytes,
        untimed_cold.resident_transport_bytes + transport
    );
    assert_eq!(
        stats.accounted_bytes,
        untimed_cold.accounted_bytes + accounted
    );
    assert_eq!(checks.len(), untimed_checks.len());
    for (actual, expected) in checks.iter().zip(&untimed_checks) {
        assert_eq!(actual.verdict(), expected.verdict());
    }
}

#[test]
#[ignore = "requires Metal: formula executes while relation columns remain prepared"]
fn metal_formula_executes_while_relation_columns_remain_prepared() {
    compose(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: formula executes while relation columns remain prepared"]
fn vulkan_formula_executes_while_relation_columns_remain_prepared() {
    compose(GpuApi::Vulkan);
}

/// Candidates of a heavy batch: the device's default batch ceiling.
const HEAVY: usize = 1024;
/// Room for a heavy batch of the wide subject, under both oracles.
const BATCH_BYTES: u64 = 768 * 1024 * 1024;

/// A tight certificate and candidates of its theory, for either oracle.
struct Subject {
    plan: TightPlan,
    candidates: Vec<Interpretation>,
}

// `width` gates on each of `levels` levels over 32 body atoms, alternating
// disjunctions and conjunctions of `fan_in` gates below: the fan-in multiplies
// device work without growing per-candidate truth storage. Each last-level gate
// is the body of a rule for its own head atom, so the program is tight. Every
// candidate holds all heads, so it is a model and its proper-subset query is
// propagated.
fn layered(levels: usize, width: usize, fan_in: usize) -> Subject {
    let body = 32;
    let mut nodes: Vec<Node> = (0..body).map(Node::atom).collect();
    let mut operands = Vec::new();
    let mut below: Vec<usize> = (0..body).collect();
    for level in 0..levels {
        let start = nodes.len();
        for index in 0..width {
            let span = zetesis_ferraris::OperandSpan {
                start: operands.len(),
                length: fan_in,
            };
            operands.extend((0..fan_in).map(|offset| below[(index + offset) % below.len()]));
            nodes.push(if level % 2 == 0 {
                Node::or_span(span)
            } else {
                Node::and_span(span)
            });
        }
        below = (start..nodes.len()).collect();
    }
    let mut roots = Vec::with_capacity(width);
    for (head, gate) in below.into_iter().enumerate() {
        nodes.push(Node::atom(body + head));
        nodes.push(Node::implies(gate, nodes.len() - 1));
        roots.push(nodes.len() - 1);
    }
    let theory = Theory::new(
        body + width,
        zetesis_ferraris::FormulaParts::new(nodes, operands).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap();
    let plan = TightPlan::compile(
        &theory,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let candidates = (0..HEAVY)
        .map(|candidate| {
            Interpretation::new(
                plan.theory(),
                (0..body)
                    .filter(|atom| (atom + candidate) % 3 == 0)
                    .chain(body..body + width),
            )
            .unwrap()
        })
        .collect();
    Subject { plan, candidates }
}

/// The timing protocol of one device oracle.
trait Timed {
    fn batch(
        &mut self,
        subject: &Subject,
        count: usize,
        control: &Cancellation,
    ) -> Result<(), Stop>;
    fn submitted(&self) -> bool;
    fn reported(&self) -> Option<BatchComputeTime>;
    fn unresolved(&self) -> bool;
    fn flush(&mut self, control: &Cancellation) -> Result<(), GpuError>;
    fn clear(&mut self);
    fn workgroup_size(&self) -> u32;
}

impl Timed for GpuFormulaOracle {
    fn batch(
        &mut self,
        subject: &Subject,
        count: usize,
        control: &Cancellation,
    ) -> Result<(), Stop> {
        let limits = FormulaLimits {
            max_batch_bytes: BATCH_BYTES,
            ..FormulaLimits::default()
        };
        self.propagate_batch_with_cancellation(
            subject.plan.theory(),
            &subject.candidates[..count],
            limits,
            control,
        )
        .map(|_| ())
        .map_err(|error| error.interruption().unwrap_or_else(|| panic!("{error}")))
    }
    fn submitted(&self) -> bool {
        self.last_submission_candidates().is_some()
    }
    fn reported(&self) -> Option<BatchComputeTime> {
        self.reported_compute_time()
    }
    fn unresolved(&self) -> bool {
        self.compute_time_unresolved()
    }
    fn flush(&mut self, control: &Cancellation) -> Result<(), GpuError> {
        self.flush_compute_time(Duration::from_secs(30), control)
    }
    fn clear(&mut self) {
        self.clear_residency();
    }
    fn workgroup_size(&self) -> u32 {
        self.last_batch_stats().unwrap().workgroup_size
    }
}

impl Timed for GpuTightOracle {
    fn batch(
        &mut self,
        subject: &Subject,
        count: usize,
        control: &Cancellation,
    ) -> Result<(), Stop> {
        let limits = TightGpuLimits {
            max_batch_bytes: BATCH_BYTES,
            ..TightGpuLimits::default()
        };
        self.check_batch(&subject.plan, &subject.candidates[..count], limits, control)
            .map(|_| ())
            .map_err(|error| match error {
                TightGpuError::Stopped(stop) => stop,
                TightGpuError::Gpu(error) => panic!("{error}"),
            })
    }
    fn submitted(&self) -> bool {
        self.activity().submissions != 0
    }
    fn reported(&self) -> Option<BatchComputeTime> {
        self.reported_compute_time()
    }
    fn unresolved(&self) -> bool {
        self.compute_time_unresolved()
    }
    fn flush(&mut self, control: &Cancellation) -> Result<(), GpuError> {
        self.flush_compute_time(Duration::from_secs(30), control)
    }
    fn clear(&mut self) {
        self.clear_residency();
    }
    fn workgroup_size(&self) -> u32 {
        self.last_batch_stats().unwrap().workgroup_size
    }
}

// Qualify the host without the per-batch placement under test: a flush after
// each batch reads its pass after observed completion, the evidenced placement.
// Heavy passes must outlast every light call, or the alternation cannot tell a
// shifted report from a true one. Returns the shortest heavy pass and the
// longest light call.
fn calibrate(oracle: &mut impl Timed, wide: &Subject, name: &str) -> (Duration, Duration) {
    let live = Cancellation::default();
    let mut shortest_heavy = Duration::MAX;
    let mut longest_light = Duration::ZERO;
    for _ in 0..2 {
        oracle.batch(wide, HEAVY, &live).unwrap();
        oracle.flush(&live).unwrap();
        let Some(BatchComputeTime::Measured(heavy)) = oracle.reported() else {
            panic!("unqualified: {name} measured no calibration heavy pass");
        };
        shortest_heavy = shortest_heavy.min(heavy);
        let started = Instant::now();
        oracle.batch(wide, 1, &live).unwrap();
        longest_light = longest_light.max(started.elapsed());
        oracle.flush(&live).unwrap();
    }
    assert!(
        shortest_heavy > longest_light,
        "unqualified: {name}'s calibrated heavy pass {shortest_heavy:?} does not outlast its light call {longest_light:?}"
    );
    (shortest_heavy, longest_light)
}

// Every completed batch is reported once, by the next submitting call or the
// flush, from its own pass. Heavy and light batches alternate, so the count
// changes on every batch; the sequence then changes the theory and, under fitted
// sizing, the workgroup size. A report shifted by one batch would put a light
// pass on each heavy batch, and a heavy interval against a light batch's window,
// where rule 6 refuses it: so every measured heavy report must exceed every
// light call, and most reports of each kind must be measured.
fn qualify_attribution(oracle: &mut impl Timed, wide: &Subject, narrow: &Subject, name: &str) {
    let (calibrated_heavy, calibrated_light) = calibrate(oracle, wide, name);
    let sequence = [
        (wide, HEAVY),
        (wide, 1),
        (wide, HEAVY),
        (wide, 1),
        (wide, HEAVY),
        (wide, 1),
        (narrow, 1),
        (narrow, HEAVY),
        (wide, HEAVY),
        (wide, 1),
    ];
    let live = Cancellation::default();
    let mut calls = Vec::new();
    let mut reports = Vec::new();
    let mut sizes = Vec::new();
    for (index, (subject, count)) in sequence.into_iter().enumerate() {
        let started = Instant::now();
        oracle.batch(subject, count, &live).unwrap();
        calls.push(started.elapsed());
        sizes.push(oracle.workgroup_size());
        // A batch after a flush has nothing to report; every later call
        // reports the batch before it.
        assert_eq!(
            oracle.reported().is_some(),
            index != 0,
            "{name} call {index}"
        );
        reports.extend(oracle.reported());
        assert!(oracle.unresolved());
    }
    oracle.flush(&live).unwrap();
    reports.push(oracle.reported().unwrap());
    assert!(!oracle.unresolved());
    assert_eq!(reports.len(), sequence.len());
    // Rule 6 bounds every measured pass by its own submission's host window,
    // which lies inside the call that ran the batch.
    for (batch, report) in reports.iter().enumerate() {
        if let BatchComputeTime::Measured(elapsed) = report {
            assert!(!elapsed.is_zero());
            assert!(
                *elapsed <= calls[batch],
                "rule 6 violated: {name} batch {batch} reported {elapsed:?}, longer than its call {:?}",
                calls[batch]
            );
        }
    }
    let alternation = |count| {
        sequence
            .iter()
            .enumerate()
            .filter(move |(_, (subject, size))| std::ptr::eq(*subject, wide) && *size == count)
            .map(|(batch, _)| batch)
    };
    let longest_light = alternation(1).map(|batch| calls[batch]).max().unwrap();
    for (kind, count) in [("heavy", HEAVY), ("light", 1)] {
        let measured = alternation(count)
            .filter(|batch| matches!(reports[*batch], BatchComputeTime::Measured(_)))
            .count();
        assert!(
            measured >= 3,
            "misattribution suspected: {name} measured {measured} of 4 {kind} batches; reports {reports:?}"
        );
    }
    for batch in alternation(HEAVY) {
        if let BatchComputeTime::Measured(heavy) = reports[batch] {
            assert!(
                heavy > longest_light,
                "misattribution: {name} heavy batch {batch} reported {heavy:?}, not more than the longest light call {longest_light:?}, though calibrated heavy passes ({calibrated_heavy:?}) outlast light calls ({calibrated_light:?}); reports {reports:?}"
            );
        }
    }
    // The narrow theory keeps the base size; the wide one fits a larger one.
    assert!(
        sizes[6] < sizes[5] && sizes[8] == sizes[5],
        "{name} sizes {sizes:?}"
    );
    let measured = reports
        .iter()
        .filter(|report| matches!(report, BatchComputeTime::Measured(_)))
        .count();
    println!(
        "{name} timing: measured={measured} unavailable={} calibrated heavy pass={calibrated_heavy:?} light call={calibrated_light:?} longest alternation light call={longest_light:?} sizes={sizes:?}",
        reports.len() - measured
    );
}

// A control stop between batches submits nothing and loses nothing: the call
// and the flush leave the completed batch unresolved on a healthy context, and
// the next call or a live flush reports it. Clearing residency reports an
// unresolved batch unavailable, and the next call starts a fresh timer.
fn qualify_stops_and_release(oracle: &mut impl Timed, subject: &Subject, name: &str) {
    let live = Cancellation::default();
    let fired = Cancellation::default();
    fired.cancel();
    oracle.batch(subject, 1, &live).unwrap();
    assert!(oracle.unresolved());
    assert_eq!(oracle.batch(subject, 1, &fired), Err(Stop::Cancelled));
    assert!(!oracle.submitted(), "{name}");
    assert_eq!(oracle.reported(), None);
    assert!(oracle.unresolved());
    let stopped = oracle.flush(&fired).unwrap_err();
    assert_eq!(stopped.interruption(), Some(Stop::Cancelled));
    assert_eq!(oracle.reported(), None);
    assert!(oracle.unresolved());
    oracle.batch(subject, 1, &live).unwrap();
    assert!(oracle.reported().is_some(), "{name}");
    oracle.flush(&live).unwrap();
    assert!(oracle.reported().is_some(), "{name}");
    assert!(!oracle.unresolved());
    // An empty flush neither submits nor reports.
    oracle.flush(&live).unwrap();
    assert_eq!(oracle.reported(), None);

    oracle.batch(subject, 1, &live).unwrap();
    oracle.clear();
    assert_eq!(oracle.reported(), Some(BatchComputeTime::Unavailable));
    assert!(!oracle.unresolved());
    oracle.batch(subject, 1, &live).unwrap();
    assert_eq!(
        oracle.reported(),
        None,
        "{name}: a fresh timer reports nothing"
    );
    oracle.flush(&live).unwrap();
    assert!(oracle.reported().is_some());
}

fn attribute(backend: GpuApi) {
    let context =
        GpuContext::new_selected_with_timing(GpuOptions::default(), GpuSelection { api: backend })
            .unwrap();
    physical::verify(backend, context.info());
    assert!(
        qualify_timing(&context),
        "unqualified: compute-pass timestamps are not supported here"
    );
    let limits = context.limits();
    assert!(
        limits
            .max_compute_invocations_per_workgroup
            .min(limits.max_compute_workgroup_size_x)
            >= 256,
        "the fitted sizes assume a device granting 256 invocations"
    );
    let wide = layered(128, 256, 8);
    let narrow = layered(64, 32, 3);
    let mut formula = GpuFormulaOracle::from_context(&context).unwrap();
    qualify_attribution(&mut formula, &wide, &narrow, "formula");
    qualify_stops_and_release(&mut formula, &narrow, "formula");
    let mut tight = GpuTightOracle::from_context(&context).unwrap();
    qualify_attribution(&mut tight, &wide, &narrow, "tight");
    qualify_stops_and_release(&mut tight, &narrow, "tight");
}

#[test]
#[ignore = "requires Metal: compute times belong to their own batches"]
fn metal_compute_times_belong_to_their_own_batches() {
    attribute(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: compute times belong to their own batches"]
fn vulkan_compute_times_belong_to_their_own_batches() {
    attribute(GpuApi::Vulkan);
}
