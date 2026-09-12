//! Static readback contracts exercised without constructing a device.
//! CPU closures supply valid payloads; mutations change only their transport
//! representation. These checks do not simulate a driver or qualify hardware.

use std::num::NonZeroU32;

use super::{BatchPlan, GraphPlan, PackedSeeds, decode, next_epoch};
use crate::{GpuCheck, GpuErrorKind, GpuLimits};
use zetesis_core::{
    AdmissionLimits, AtomPattern, GroundProgram, Predicate, Program, Seed, StaticLimits, Template,
};
use zetesis_cpu::{Control, Limits, check_static};

fn compile(templates: Vec<Template>) -> GroundProgram {
    let program = Program::new(templates, AdmissionLimits::default()).unwrap();
    GroundProgram::compile(&program, StaticLimits::default()).unwrap()
}

fn atom(name: &str) -> AtomPattern {
    AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}

fn fixture() -> GroundProgram {
    let a = atom("a");
    let b = atom("b");
    let c = atom("c");
    compile(vec![
        Template::new(Some(a.clone()), vec![], vec![a.clone()], vec![], vec![]),
        Template::new(Some(b), vec![a.clone()], vec![], vec![], vec![]),
        Template::new(None, vec![a], vec![], vec![], vec![]),
        Template::new(None, vec![c.clone()], vec![c], vec![], vec![]),
    ])
}

fn seeds(graph: &GroundProgram) -> Vec<Seed> {
    [vec![], vec![0], vec![2], vec![0, 2]]
        .into_iter()
        .map(|ids| {
            Seed::new(
                graph.program(),
                ids.into_iter().map(|id| graph.atoms()[id].clone()),
            )
            .unwrap()
        })
        .collect()
}

fn plan(graph: &GroundProgram, candidates: usize) -> BatchPlan {
    let device = wgpu::Limits::default();
    BatchPlan::for_graph(
        &GraphPlan::new(graph, &device).unwrap(),
        candidates,
        GpuLimits::default(),
        &device,
        true,
        NonZeroU32::new(7).unwrap(),
    )
    .unwrap()
}

fn reply(graph: &GroundProgram, seeds: &[Seed], plan: &BatchPlan) -> Vec<u32> {
    seeds
        .iter()
        .enumerate()
        .flat_map(|(world, seed)| {
            let check = check_static(graph, seed, Limits::default(), &Control::default()).unwrap();
            let status =
                u32::from(check.constraint_violated()) | (u32::from(check.seed_mismatch()) << 1);
            [
                plan.epoch.get(),
                u32::try_from(world).unwrap(),
                status,
                0x5352_4331,
            ]
            .into_iter()
            .chain(check.closure_words().iter().copied())
            .collect::<Vec<_>>()
        })
        .collect()
}

fn assert_matches_cpu(graph: &GroundProgram, seeds: &[Seed], checks: &[GpuCheck]) {
    assert_eq!(checks.len(), seeds.len());
    for (seed, actual) in seeds.iter().zip(checks) {
        let expected = check_static(graph, seed, Limits::default(), &Control::default()).unwrap();
        assert_eq!(actual.closure_words(), expected.closure_words());
        assert_eq!(actual.accepted(), expected.accepted());
        assert_eq!(actual.constraint_violated(), expected.constraint_violated());
        assert_eq!(actual.seed_mismatch(), expected.seed_mismatch());
    }
}

#[test]
fn complete_records_preserve_every_rejection_reason() {
    let graph = fixture();
    let seeds = seeds(&graph);
    let plan = plan(&graph, seeds.len());
    let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
    let records = reply(&graph, &seeds, &plan);
    let checks = decode(&records, &plan, &packed).unwrap();
    assert_matches_cpu(&graph, &seeds, &checks);
    assert_eq!(
        checks.iter().map(|check| check.status).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
}

#[test]
fn zero_atom_worlds_still_require_complete_records() {
    for graph in [
        compile(vec![]),
        compile(vec![Template::new(None, vec![], vec![], vec![], vec![])]),
    ] {
        let seeds = [Seed::new(graph.program(), []).unwrap()];
        let plan = plan(&graph, seeds.len());
        let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
        let records = reply(&graph, &seeds, &plan);
        assert_eq!(records.len(), 4);
        assert_matches_cpu(&graph, &seeds, &decode(&records, &plan, &packed).unwrap());
        assert_eq!(
            decode(&[0; 4], &plan, &packed).unwrap_err().kind(),
            GpuErrorKind::Readback
        );
    }
}

#[test]
fn an_unwritten_record_is_never_logical_acceptance() {
    // The old zero-status layout admitted an empty closure for this fact.
    let graph = compile(vec![Template::new(
        Some(atom("p")),
        vec![],
        vec![],
        vec![],
        vec![],
    )]);
    let seeds = [Seed::new(graph.program(), []).unwrap()];
    let plan = plan(&graph, 1);
    let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
    let records = reply(&graph, &seeds, &plan);
    assert_matches_cpu(&graph, &seeds, &decode(&records, &plan, &packed).unwrap());
    assert_eq!(
        decode(&vec![0; records.len()], &plan, &packed)
            .unwrap_err()
            .kind(),
        GpuErrorKind::Readback
    );
}

#[test]
fn one_missing_record_refuses_the_entire_batch() {
    let graph = fixture();
    let seeds = seeds(&graph);
    let plan = plan(&graph, seeds.len());
    let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
    let complete = reply(&graph, &seeds, &plan);
    let stride = 4 + graph.word_count();
    for world in 0..seeds.len() {
        let mut records = complete.clone();
        records[world * stride..][..stride].fill(0);
        assert_eq!(
            decode(&records, &plan, &packed).unwrap_err().kind(),
            GpuErrorKind::Readback
        );
    }
}

#[test]
fn every_record_requires_the_current_epoch() {
    let graph = fixture();
    let seeds = seeds(&graph);
    let plan = plan(&graph, seeds.len());
    let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
    let complete = reply(&graph, &seeds, &plan);
    let stride = 4 + graph.word_count();
    for world in 0..seeds.len() {
        for epoch in [0, 6, 8, u32::MAX] {
            let mut records = complete.clone();
            records[world * stride] = epoch;
            assert_eq!(
                decode(&records, &plan, &packed).unwrap_err().kind(),
                GpuErrorKind::Readback
            );
        }
    }
}

#[test]
fn every_record_requires_the_completion_marker() {
    let graph = fixture();
    let seeds = seeds(&graph);
    let plan = plan(&graph, seeds.len());
    let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
    let complete = reply(&graph, &seeds, &plan);
    let stride = 4 + graph.word_count();
    for world in 0..seeds.len() {
        for marker in [0, 1, 0x5352_4330, u32::MAX] {
            let mut records = complete.clone();
            records[world * stride + 3] = marker;
            assert_eq!(
                decode(&records, &plan, &packed).unwrap_err().kind(),
                GpuErrorKind::Readback
            );
        }
    }
}

#[test]
fn records_cannot_be_reordered() {
    let graph = fixture();
    let seeds = seeds(&graph);
    let plan = plan(&graph, seeds.len());
    let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
    let complete = reply(&graph, &seeds, &plan);
    let stride = 4 + graph.word_count();
    for world in 1..seeds.len() {
        let mut records = complete.clone();
        for word in 0..stride {
            records.swap(word, world * stride + word);
        }
        assert_eq!(
            decode(&records, &plan, &packed).unwrap_err().kind(),
            GpuErrorKind::Readback
        );
    }
}

#[test]
fn a_duplicate_record_cannot_replace_another_world() {
    let graph = fixture();
    let seeds = seeds(&graph);
    let plan = plan(&graph, seeds.len());
    let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
    let complete = reply(&graph, &seeds, &plan);
    let stride = 4 + graph.word_count();
    for world in 1..seeds.len() {
        let mut records = complete.clone();
        records[world * stride..][..stride].copy_from_slice(&complete[..stride]);
        assert_eq!(
            decode(&records, &plan, &packed).unwrap_err().kind(),
            GpuErrorKind::Readback
        );
    }
}

#[test]
fn readback_requires_the_exact_dispatched_shape() {
    let graph = fixture();
    let seeds = seeds(&graph);
    let plan = plan(&graph, seeds.len());
    let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
    let complete = reply(&graph, &seeds, &plan);
    for length in 0..complete.len() {
        assert_eq!(
            decode(&complete[..length], &plan, &packed)
                .unwrap_err()
                .kind(),
            GpuErrorKind::Readback
        );
    }
    let mut extra = complete.clone();
    extra.push(0);
    assert_eq!(
        decode(&extra, &plan, &packed).unwrap_err().kind(),
        GpuErrorKind::Readback
    );
}

#[test]
fn unused_closure_tail_bits_are_refused() {
    let graph = fixture();
    let seeds = seeds(&graph);
    let plan = plan(&graph, seeds.len());
    let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
    let complete = reply(&graph, &seeds, &plan);
    let stride = 4 + graph.word_count();
    for world in 0..seeds.len() {
        for bit in graph.atom_count()..32 {
            let mut records = complete.clone();
            records[world * stride + stride - 1] |= 1 << bit;
            assert_eq!(
                decode(&records, &plan, &packed).unwrap_err().kind(),
                GpuErrorKind::Readback
            );
        }
    }
}

#[test]
fn unknown_verdict_bits_are_refused() {
    let graph = fixture();
    let seeds = seeds(&graph);
    let plan = plan(&graph, seeds.len());
    let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
    let complete = reply(&graph, &seeds, &plan);
    let stride = 4 + graph.word_count();
    for bit in 2..32 {
        let mut records = complete.clone();
        records[2] |= 1 << bit;
        records[stride + 2] |= 1 << bit;
        assert_eq!(
            decode(&records, &plan, &packed).unwrap_err().kind(),
            GpuErrorKind::Readback
        );
    }
}

#[test]
fn gate_projection_is_checked_against_the_original_seed() {
    let graph = fixture();
    let seeds = seeds(&graph);
    let plan = plan(&graph, seeds.len());
    let packed = PackedSeeds::new(&graph, seeds.iter().map(Seed::view), &plan).unwrap();
    let complete = reply(&graph, &seeds, &plan);
    let stride = 4 + graph.word_count();
    for world in 0..seeds.len() {
        let mut records = complete.clone();
        records[world * stride + 2] ^= 2;
        assert_eq!(
            decode(&records, &plan, &packed).unwrap_err().kind(),
            GpuErrorKind::Readback
        );
    }
}

#[test]
fn submission_epochs_never_wrap_or_become_zero() {
    assert_eq!(next_epoch(0).unwrap().get(), 1);
    assert_eq!(next_epoch(u32::MAX - 1).unwrap().get(), u32::MAX);
    assert_eq!(
        next_epoch(u32::MAX).unwrap_err().kind(),
        GpuErrorKind::Capacity
    );
}
