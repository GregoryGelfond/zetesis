//! Synthetic telemetry tests validate the protocol, never physical GPU execution.

use std::fmt::Write as _;

use crate::corpus_comparison::normalize::{self, Answer};
use zetesis_backend::{Backend, GpuApi};

const GOOD: &str = include_str!("formula_statistics.txt");

fn answer() -> Answer {
    normalize::native(
        "Answer: 1\na\nSATISFIABLE\nCoverage: exhausted\nModels: 1\n",
        false,
    )
    .unwrap()
}

fn outer_unsat() -> String {
    GOOD.replace(
        "batches=1; candidates=1; propagation work=12; completed sweeps=1; GPU-decided committed=1",
        "batches=0; candidates=0; propagation work=0; completed sweeps=0; GPU-decided committed=0",
    )
    .replace("peak authored GPU bytes=128", "peak authored GPU bytes=0")
    .replace("candidates=1; queries=0", "candidates=0; queries=0")
    .replace("verified stable models=1", "verified stable models=0")
    .replace("classical queries=2", "classical queries=1")
}

#[test]
fn completed_dispatches_preserve_actual_adapter_and_accounting() {
    let record = super::formula(GOOD, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer()).unwrap();
    assert_eq!(
        record.status,
        crate::corpus_comparison::execution::Status::GpuExercised
    );
    assert_eq!(record.api, "Metal");
    assert_eq!(record.vendor_id, 0x106b);
    assert_eq!(record.adapter, "fixture adapter, Metal; vendor=0x106b");
    assert_eq!(
        (
            record.gpu_batches,
            record.gpu_candidates,
            record.gpu_decided,
            record.cpu_residuals
        ),
        (1, 1, 1, 0)
    );
    assert_eq!(
        (
            record.gpu_work,
            record.gpu_sweeps,
            record.peak_authored_gpu_bytes
        ),
        (12, 1, 128)
    );
}

#[test]
fn residual_completion_and_outer_unsat_have_distinct_execution_scopes() {
    let mixed = GOOD
        .replace(
            "candidates=1; propagation work=12",
            "candidates=3; propagation work=36",
        )
        .replace("CPU residuals completed=0", "CPU residuals completed=2")
        .replace(
            "candidates=1; queries=0; witnesses=0",
            "candidates=3; queries=2; witnesses=1",
        )
        .replace("verified stable models=1", "verified stable models=2")
        .replace("classical queries=2", "classical queries=4");
    let two = normalize::native(
        "Answer: 1\na\nAnswer: 2\nb\nSATISFIABLE\nCoverage: exhausted\nModels: 2\n",
        false,
    )
    .unwrap();
    let record = super::formula(&mixed, Backend::Gpu(Some(GpuApi::Metal)), 64, &two).unwrap();
    assert_eq!(
        (
            record.gpu_decided,
            record.cpu_residuals,
            record.countermodel_witnesses,
            record.verified_stable_models
        ),
        (1, 2, 1, 2)
    );
    let unsat =
        normalize::native("UNSATISFIABLE\nCoverage: exhausted\nModels: 0\n", false).unwrap();
    let record = super::formula(
        &outer_unsat(),
        Backend::Gpu(Some(GpuApi::Metal)),
        64,
        &unsat,
    )
    .unwrap();
    assert_eq!(
        record.status,
        crate::corpus_comparison::execution::Status::OuterUnsatWithoutMembership
    );
    assert_eq!(record.gpu_batches, 0);
    assert!(
        super::formula(
            &outer_unsat(),
            Backend::Gpu(Some(GpuApi::Metal)),
            64,
            &answer()
        )
        .is_err()
    );
}

#[test]
fn gpu_backends_require_their_resolved_actual_api() {
    let native = GpuApi::native();
    let foreign = match native {
        GpuApi::Metal => GpuApi::Vulkan,
        GpuApi::Vulkan => GpuApi::Metal,
    };
    for (policy, api, vendor) in [
        (Backend::Gpu(None), native.name(), "106b"),
        (Backend::Gpu(Some(GpuApi::Metal)), "Metal", "106b"),
        (Backend::Gpu(Some(GpuApi::Vulkan)), "Vulkan", "1002"),
    ] {
        let source = GOOD.replace("Metal; vendor=0x106b", &format!("{api}; vendor=0x{vendor}"));
        let record = super::formula(&source, policy, 64, &answer()).unwrap();
        assert_eq!(record.api, api);
    }
    // `gpu` accepts only the native API; the CPU accepts no device route.
    let source = GOOD.replace(
        "Metal; vendor=0x106b",
        &format!("{}; vendor=0x1002", foreign.name()),
    );
    assert!(super::formula(&source, Backend::Gpu(None), 64, &answer()).is_err());
    for policy in [Backend::Cpu, Backend::Gpu(Some(GpuApi::Vulkan))] {
        assert!(super::formula(GOOD, policy, 64, &answer()).is_err());
    }
    for invalid in [
        GOOD.replace(", Metal;", ", Noop;"),
        GOOD.replace("fixture adapter, Metal", ", Metal"),
        GOOD.replace("0x106b", "0xxyz"),
        GOOD.replace("0x106b", "0x100000000"),
        GOOD.replace("0x106b", "0x"),
        GOOD.replace("; vendor=0x106b", ""),
        GOOD.replace(", Metal; vendor", "; vendor"),
        GOOD.replace("fixture adapter", "fixture\tadapter"),
    ] {
        assert!(
            super::formula(&invalid, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer()).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn missing_duplicate_changed_and_malformed_telemetry_cannot_qualify_a_device() {
    for prefix in [
        "  completion: ",
        "  effective execution: ",
        "  formula GPU: ",
        "  formula accounting: ",
        "  formula batch limits: ",
        "  countermodel: ",
    ] {
        let missing = GOOD
            .lines()
            .filter(|line| !line.starts_with(prefix))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            super::formula(&missing, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer()).is_err()
        );
        let repeated = GOOD.lines().find(|line| line.starts_with(prefix)).unwrap();
        let duplicate = format!("{GOOD}{repeated}\n");
        assert!(
            super::formula(&duplicate, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer()).is_err()
        );
    }
    for (from, to) in [
        (
            "completion: exhausted",
            "completion: interrupted (partial coverage)",
        ),
        (
            "backend=hybrid GPU propagation + exact CPU residual search",
            "backend=cpu",
        ),
        ("oracle=countermodel", "oracle=closure"),
        ("batches=1", "batches=-1"),
        ("batches=1", "batches=+1"),
        ("batches=1", "batches=1.0"),
        ("batches=1", "batches="),
        ("batches=1", "batches=18446744073709551616"),
        ("batches=1", "submissions=1"),
        (
            "; candidates=1; propagation work=12",
            "; propagation work=12",
        ),
        (
            "; CPU residuals completed=0",
            "; CPU residuals completed=0; extra=1",
        ),
        ("candidates=64", "candidates=0"),
        ("pending bytes=4096", "pending bytes=0"),
        (" (u32 ceiling)", ""),
        (
            "propagation work/candidate=1000",
            "propagation work/candidate=4294967296",
        ),
        ("GPU kernel timing=unavailable", "GPU kernel timing=0"),
        (
            "GPU kernel timing=unavailable",
            "GPU kernel timing=unavailable; extra=1",
        ),
    ] {
        let changed = GOOD.replace(from, to);
        assert_ne!(changed, GOOD);
        assert!(
            super::formula(&changed, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer()).is_err(),
            "{from} -> {to}"
        );
    }
}

#[test]
fn uncommitted_dropped_candidates_and_impossible_resource_counts_are_rejected() {
    for (from, to) in [
        ("pending candidates=0", "pending candidates=1"),
        ("queued verified models=0", "queued verified models=1"),
        ("candidates=1; queries=0", "candidates=2; queries=0"),
        ("queries=0", "queries=1"),
        ("witnesses=0", "witnesses=1"),
        ("GPU-decided committed=1", "GPU-decided committed=0"),
        ("verified stable models=1", "verified stable models=2"),
        ("classical queries=2", "classical queries=0"),
        ("classical queries=2", "classical queries=1"),
        ("classical queries=2", "classical queries=3"),
        ("batches=1", "batches=0"),
        ("batches=1", "batches=2"),
        ("propagation work=12", "propagation work=0"),
        ("propagation work=12", "propagation work=1001"),
        ("completed sweeps=1", "completed sweeps=65"),
        ("peak authored GPU bytes=128", "peak authored GPU bytes=0"),
        (
            "peak authored GPU bytes=128",
            "peak authored GPU bytes=4097",
        ),
    ] {
        let changed = GOOD.replace(from, to);
        assert!(
            super::formula(&changed, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer()).is_err(),
            "{from} -> {to}"
        );
    }
    assert!(super::formula(GOOD, Backend::Gpu(Some(GpuApi::Metal)), 32, &answer()).is_err());
    let mut incorrect = answer();
    incorrect.model_count = 2;
    assert!(super::formula(GOOD, Backend::Gpu(Some(GpuApi::Metal)), 64, &incorrect).is_err());
}

fn optimal_output(display: &str, ties: usize) -> String {
    let mut output = String::new();
    for index in 1..=ties {
        writeln!(output, "Answer: {index}\n{display}\nOptimization: 1 7").unwrap();
    }
    writeln!(output, "OPTIMUM FOUND\nCoverage: exhausted\nModels: {ties}").unwrap();
    output
}

#[test]
fn verified_incumbents_can_exceed_final_ties_without_collapsing_hidden_displays() {
    // Three candidates were proved stable: two by propagation and one by exact
    // residual search. Optimization can discard the earlier worse incumbent;
    // display still cannot reveal the identities of the remaining hidden atoms.
    let stats = GOOD
        .replace(
            "candidates=1; propagation work=12",
            "candidates=3; propagation work=36",
        )
        .replace("GPU-decided committed=1", "GPU-decided committed=2")
        .replace("CPU residuals completed=0", "CPU residuals completed=1")
        .replace("candidates=1; queries=0", "candidates=3; queries=1")
        .replace("verified stable models=1", "verified stable models=3")
        .replace("classical queries=2", "classical queries=4");
    for (display, symbols) in [
        ("", vec![]),
        ("shown shown", vec!["shown".to_owned(), "shown".to_owned()]),
    ] {
        let optimal = normalize::native(&optimal_output(display, 2), true).unwrap();
        assert_eq!(optimal.cost, Some(vec![1, 7]));
        assert_eq!(optimal.model_count, 2);
        assert_eq!(
            optimal.models.len(),
            1,
            "equal displays do not collapse two models"
        );
        assert_eq!(optimal.model_multiplicities, vec![(symbols, 2)]);
        let record =
            super::formula(&stats, Backend::Gpu(Some(GpuApi::Metal)), 64, &optimal).unwrap();
        assert_eq!(record.verified_stable_models, 3);
        assert_eq!((record.gpu_decided, record.cpu_residuals), (2, 1));
        let impossible = normalize::native(&optimal_output(display, 4), true).unwrap();
        assert_eq!(impossible.model_count, 4);
        assert!(
            super::formula(&stats, Backend::Gpu(Some(GpuApi::Metal)), 64, &impossible).is_err(),
            "four final ties cannot come from only three verified stable models"
        );
    }
    let unoptimized = normalize::native(
        "Answer: 1\na\nAnswer: 2\nb\nSATISFIABLE\nCoverage: exhausted\nModels: 2\n",
        false,
    )
    .unwrap();
    assert!(
        super::formula(&stats, Backend::Gpu(Some(GpuApi::Metal)), 64, &unoptimized).is_err(),
        "complete unoptimized enumeration has no discarded incumbents"
    );
}

#[test]
fn outer_unsat_without_membership_cannot_claim_unaccounted_device_work() {
    let answer =
        normalize::native("UNSATISFIABLE\nCoverage: exhausted\nModels: 0\n", false).unwrap();
    let valid = outer_unsat();
    for (from, to) in [
        ("batches=0", "batches=1"),
        ("propagation work=0", "propagation work=1"),
        ("completed sweeps=0", "completed sweeps=1"),
        ("peak authored GPU bytes=0", "peak authored GPU bytes=128"),
    ] {
        let invalid = valid.replace(from, to);
        let error =
            super::formula(&invalid, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer).unwrap_err();
        assert!(error.contains("zero-dispatch"), "{from}: {error}");
    }
    assert_eq!(
        super::formula(&valid, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer)
            .unwrap()
            .status,
        crate::corpus_comparison::execution::Status::OuterUnsatWithoutMembership
    );
}

#[test]
fn truncated_or_nonnumeric_resource_records_never_supply_missing_allowances() {
    let prefix = "  formula batch limits: ";
    let original = GOOD.lines().find(|line| line.starts_with(prefix)).unwrap();
    // The solver's answer can be complete while a statistics record is only
    // partly written. Every required allowance must still be present and typed.
    for shortened in [
        "",
        "candidates=64",
        "candidates=64; pending bytes=4096",
        "candidates=64; pending bytes=4096; propagation sweeps=64",
        "candidates=64; pending bytes=4096; propagation sweeps=64; propagation work/candidate=1000 (u32 ceiling)",
    ] {
        let invalid = GOOD.replace(original, &format!("{prefix}{shortened}"));
        assert!(
            super::formula(&invalid, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer()).is_err(),
            "{shortened}"
        );
    }
    for (from, to) in [
        ("candidates=64", "candidates=unknown"),
        ("pending bytes=4096", "pending bytes=18446744073709551616"),
        ("propagation sweeps=64", "propagation sweeps=-1"),
        (
            "propagation work/candidate=1000",
            "propagation work/candidate=+1000",
        ),
        ("; CPU residuals completed=0", ""),
        ("; verified stable models=1", ""),
    ] {
        let invalid = GOOD.replace(from, to);
        assert!(
            super::formula(&invalid, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer()).is_err(),
            "{from}"
        );
    }
    assert!(super::formula(GOOD, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer()).is_ok());
}

const CURRENT: &str = include_str!("formula_statistics_completion.txt");

fn request(workers: usize, max_scratch_bytes: u64) -> super::CompletionRequest {
    super::CompletionRequest {
        workers: workers.try_into().unwrap(),
        max_scratch_bytes,
    }
}

#[test]
fn current_completion_telemetry_preserves_legacy_without_inventing_bounded_history() {
    let legacy = super::formula(GOOD, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer()).unwrap();
    assert!(legacy.completion.is_none());
    for settings in [request(4, 268_435_456), request(1, 512)] {
        assert!(
            super::formula_for_request(
                GOOD,
                Backend::Gpu(Some(GpuApi::Metal)),
                64,
                settings,
                &answer()
            )
            .is_err()
        );
    }
    let record = super::formula_for_request(
        CURRENT,
        Backend::Gpu(Some(GpuApi::Metal)),
        64,
        request(4, 268_435_456),
        &answer(),
    )
    .unwrap();
    let completion = record.completion.unwrap();
    assert_eq!(
        record.status,
        crate::corpus_comparison::execution::Status::GpuExercised
    );
    assert_eq!(completion.profile, "bounded_completion_v1");
    assert_eq!(
        (
            completion.requested_workers,
            completion.peak_effective_workers
        ),
        (4, 0)
    );
    assert_eq!(
        (
            completion.entered,
            completion.completed,
            completion.residuals
        ),
        (1, 1, 0)
    );
    assert_eq!(completion.peak_admitted_logical_scratch_bytes, 512);
    assert_eq!(completion.max_logical_scratch_bytes, 268_435_456);
    assert_eq!(record.peak_authored_gpu_bytes, 128);
    for settings in [request(2, 268_435_456), request(4, 4096)] {
        assert!(
            super::formula_for_request(
                CURRENT,
                Backend::Gpu(Some(GpuApi::Metal)),
                64,
                settings,
                &answer()
            )
            .is_err()
        );
    }
}

#[test]
fn current_residual_and_outer_unsat_accounting_reconcile_with_native_models() {
    let mixed = CURRENT
        .replace("peak effective workers=0", "peak effective workers=2")
        .replace(
            "entered=1; residuals entered=0; completed before commit=1",
            "entered=3; residuals entered=2; completed before commit=3",
        )
        .replace("completed locally=0", "completed locally=2")
        .replace(
            "candidates=1; propagation work=12",
            "candidates=3; propagation work=36",
        )
        .replace("CPU residuals completed=0", "CPU residuals completed=2")
        .replace(
            "candidates=1; queries=0; witnesses=0",
            "candidates=3; queries=2; witnesses=1",
        )
        .replace("verified stable models=1", "verified stable models=2")
        .replace("classical queries=2", "classical queries=4");
    let two = normalize::native(
        "Answer: 1\na\nAnswer: 2\nb\nSATISFIABLE\nCoverage: exhausted\nModels: 2\n",
        false,
    )
    .unwrap();
    let record = super::formula(&mixed, Backend::Gpu(Some(GpuApi::Metal)), 64, &two).unwrap();
    let completion = record.completion.unwrap();
    assert_eq!(
        (
            completion.peak_effective_workers,
            completion.residual_completed,
            completion.failed
        ),
        (2, 2, 0)
    );
    let unsat = CURRENT.replace("entered=1; residuals entered=0; completed before commit=1", "entered=0; residuals entered=0; completed before commit=0")
        .replace("peak admitted logical scratch bytes=512", "peak admitted logical scratch bytes=0")
        .replace("batches=1; candidates=1; propagation work=12; completed sweeps=1; GPU-decided committed=1", "batches=0; candidates=0; propagation work=0; completed sweeps=0; GPU-decided committed=0")
        .replace("peak authored GPU bytes=128", "peak authored GPU bytes=0")
        .replace("candidates=1; queries=0", "candidates=0; queries=0")
        .replace("verified stable models=1", "verified stable models=0")
        .replace("classical queries=2", "classical queries=1");
    let no_models =
        normalize::native("UNSATISFIABLE\nCoverage: exhausted\nModels: 0\n", false).unwrap();
    let record = super::formula(&unsat, Backend::Gpu(Some(GpuApi::Metal)), 64, &no_models).unwrap();
    assert_eq!(
        record.status,
        crate::corpus_comparison::execution::Status::OuterUnsatWithoutMembership
    );
    assert_eq!(
        record
            .completion
            .unwrap()
            .peak_admitted_logical_scratch_bytes,
        0
    );
    assert!(
        super::formula(
            &mixed.replace("peak effective workers=2", "peak effective workers=0"),
            Backend::Gpu(Some(GpuApi::Metal)),
            64,
            &two
        )
        .is_err()
    );
}

#[test]
fn current_partial_duplicate_mixed_and_contradictory_statistics_are_not_qualification() {
    for (from, to) in [
        (
            "CPU completion requested workers=4",
            "CPU completion requested workers=0",
        ),
        ("peak effective workers=0", "peak effective workers=5"),
        ("entered=1;", "entered=2;"),
        ("completed before commit=1", "completed before commit=0"),
        ("residuals entered=0", "residuals entered=1"),
        ("completed locally=0", "completed locally=1"),
        ("failed=0", "failed=1"),
        (
            "peak admitted logical scratch bytes=512",
            "peak admitted logical scratch bytes=0",
        ),
        ("scratch limit=268435456", "scratch limit=511"),
        ("counters overflowed=false", "counters overflowed=true"),
        (
            "CPU completion requested workers=4; peak effective workers=0",
            "CPU search workers=1",
        ),
        (
            "propagation work/candidate=1000",
            "propagation work/candidate=4294967296",
        ),
        (
            "GPU kernel timing=unavailable",
            "GPU kernel timing=unavailable; extra=1",
        ),
        ("pending bytes=4096", "pending bytes=0"),
        (
            "peak authored GPU bytes=128",
            "peak authored GPU bytes=4097",
        ),
    ] {
        assert!(
            super::formula(
                &CURRENT.replace(from, to),
                Backend::Gpu(Some(GpuApi::Metal)),
                64,
                &answer()
            )
            .is_err(),
            "{from} -> {to}"
        );
    }
    for prefix in [
        "  formula completion: ",
        "  formula residual completion: ",
        "  formula accounting: ",
        "  formula GPU limits: ",
        "  formula batch limits: ",
    ] {
        let record = CURRENT.lines().find(|l| l.starts_with(prefix)).unwrap();
        for altered in [
            CURRENT.replace(record, ""),
            format!("{CURRENT}{record}\n"),
            CURRENT.replace(record, prefix),
        ] {
            assert!(
                super::formula(&altered, Backend::Gpu(Some(GpuApi::Metal)), 64, &answer()).is_err(),
                "{prefix}"
            );
        }
    }
}
