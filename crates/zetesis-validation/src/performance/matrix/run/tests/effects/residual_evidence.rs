//! Actual child processes exercise negotiation, byte accounting and publication.
//! The fixture emits prescribed observations; it never claims physical GPU work.
use super::*;
use crate::performance::matrix::{ArtifactOutcome, ProfileEvidence};
use zetesis_backend::residual_evidence::{Batch, Header};

fn zero_metric(eligible: usize) -> Value {
    if eligible == 0 {
        json!({"eligible":0,"known":0,"retained":0,"sum":0,"maximum":null,
            "median":null,"p90":null,"quantiles":"unavailable"})
    } else {
        json!({"eligible":eligible,"known":eligible,"retained":eligible,"sum":0,
            "maximum":0,"median":0,"p90":0,"quantiles":"exact"})
    }
}

fn empty_batch_summary() -> Value {
    let mut group = json!({});
    for key in [
        "open_atoms",
        "visited",
        "attempted_splits",
        "published_splits",
        "depth",
        "work",
        "query_ns",
    ] {
        group[key] = zero_metric(0);
    }
    for key in ["residuals", "width", "batch_ns"] {
        group[key] = zero_metric(1);
    }
    json!({"version":1,"algorithm":"splitmix64-reject-v1","seed":19,
        "byte_limit":8192,"retained_bytes":64,"peak_bytes":4096,
        "worlds":{"seen":0,"capacity":1,"retained":0},
        "batches":{"seen":1,"capacity":1,"retained":1},
        "groups":{"all":group,"fixed_point":group,"round_limit":group,"work_limit":group}})
}

fn producer(f: &Fixture, announces: bool, writes: bool) -> usize {
    let (mut doc, stderr) = crate::performance::matrix::fixtures::formula_fixture();
    let native: Value = serde_json::from_slice(sample().capture().unwrap().stdout()).unwrap();
    for key in ["schema", "format", "models", "outcome"] {
        doc[key] = native[key].clone();
    }
    let execution = &mut doc["statistics"]["execution"];
    execution["gpu_decided"] = json!(2);
    execution["cpu_residuals"] = json!(0);
    execution["completion"]["residuals"] = json!(0);
    execution["completion"]["residual_completed"] = json!(0);
    execution["gpu_residual_handovers"] = json!({
        "attempts":1,"incomplete_attempts":0,"committed_slates":1,"zero_residual_slates":1,
        "query_work":0,"complete_work":true,"counts_exact":true,"last_attempt":{
            "ordinal":1,"slate_sequence":0,"retry":false,"decode_complete":true,"committed":true,
            "residuals":{"fixed_point":0,"round_limit":0,"work_limit":0},"open_atoms":0,
            "peak_observation_bytes":0,"search":null},"samples":empty_batch_summary()});
    let stdout = serde_json::to_string(&doc).unwrap();
    let template = f.directory.path().join("template");
    let header = Header {
        nonce: [0; 16],
        seed: 19,
        worlds: Some(0),
        batches: Some(1),
        retained_worlds: 0,
        retained_batches: 1,
        complete: true,
    };
    let batch = Batch {
        residuals: [Some(0); 4],
        width: Some(0),
        host_ns: [Some(0); 4],
    };
    let mut payload = header.encode().to_vec();
    payload.extend(batch.encode());
    fs::write(&template, payload).unwrap();
    let help = if announces {
        "      --residual-evidence-request <FILE>"
    } else {
        "legacy solver removed --residual-evidence-request <FILE> in this build"
    };
    let emit = if writes {
        format!(
            r#"out=${{request%/*}}/samples.bin
cp {} "$out" || exit 80
nonce=$(sed -n 's/.*"nonce":\[\([^]]*\)\].*/\1/p' "$request" | tr ',' ' ')
for b in $nonce; do printf "\\$(printf '%03o' "$b")"; done | dd of="$out" bs=1 seek=24 conv=notrunc 2>/dev/null
"#,
            quote(template.to_str().unwrap())
        )
    } else {
        String::new()
    };
    let script = format!(
        r#"#!/bin/sh
for arg do
    case "$arg" in --version) echo fixture; exit 0;; --help-all) printf '%s\n' '{help}'; exit 0;; esac
done
request=
while [ "$#" -gt 0 ]; do
    if [ "$1" = --residual-evidence-request ]; then request=$2; shift; fi
    shift
done
if [ -n "$request" ]; then
{emit}
:
fi
printf '%s' {}
printf '%s' {} >&2
"#,
        quote(&stdout),
        quote(&stderr)
    );
    fs::write(&f.native, script).unwrap();
    stdout.len() + stderr.len()
}

fn plan() -> Plan {
    let profile = crate::selected::NativeExecution {
        backend: zetesis_backend::Backend::Gpu(Some(zetesis_backend::GpuApi::Metal)),
        ..Default::default()
    };
    Plan::qualification(Suite::Corpus, vec![profile], NonZeroUsize::new(1).unwrap())
        .unwrap()
        .with_residual_evidence("test-fixtures".into())
        .unwrap()
}

fn evidence_run(f: &Fixture) -> Request<'_> {
    Request {
        plan: plan(),
        ..f.request()
    }
}

#[test]
fn complete_child_artifacts_form_a_separate_untimed_pool() {
    let f = Fixture::new();
    producer(&f, true, true);
    let request = evidence_run(&f);
    let report =
        crate::performance::matrix::run_workloads(&request, &[f.unchanged(), f.workload.clone()])
            .unwrap();
    assert!(report.passed(), "{:#?}", report.residual_evidence());
    assert!(
        report
            .samples()
            .iter()
            .all(|s| s.slot().phase == Phase::Qualification)
    );
    assert!(report.summary().cells.iter().all(|c| c.timing.is_none()));
    let evidence = report.residual_evidence().unwrap();
    assert!(matches!(
        evidence.pools[0],
        ProfileEvidence::Complete { .. }
    ));
    let mut handles = Vec::new();
    for m in &evidence.members {
        let ArtifactOutcome::Retained {
            path,
            bytes,
            sha256,
            nonce,
        } = &m.artifact
        else {
            panic!("missing artifact")
        };
        assert_eq!(*bytes, 240);
        let path = f.report.parent().unwrap().join(path);
        let directory = fs::metadata(path.parent().unwrap()).unwrap();
        assert_eq!(directory.permissions().mode() & 0o077, 0);
        let raw = fs::read(&path).unwrap();
        assert_eq!(&raw[24..40], nonce);
        assert_eq!(crate::digest::sha256(&raw), *sha256);
        handles.push(path);
    }
    assert_ne!(handles[0], handles[1]);
    let ordinary_bytes = report
        .metadata()
        .chain(report.samples().iter().filter_map(Sample::capture))
        .map(|capture| capture.stdout().len() + capture.stderr().len())
        .sum::<usize>();
    assert_eq!(report.total_capture_bytes(), ordinary_bytes + 480);
    report.publish().unwrap();
    drop(report);
    assert!(handles.iter().all(|p| p.is_file()));
}

#[test]
fn legacy_child_retains_its_success_without_an_artifact() {
    let f = Fixture::new();
    producer(&f, false, false);
    let report = run_unchanged(&f, &evidence_run(&f));
    assert!(!report.passed());
    assert!(
        report
            .samples()
            .iter()
            .all(|s| s.decision == Decision::Pass)
    );
    assert!(
        matches!(&report.residual_evidence().unwrap().members[0].artifact,
        ArtifactOutcome::Unavailable{detail} if detail.contains("advertise"))
    );
}

#[test]
fn missing_artifact_cannot_replace_ordinary_qualification() {
    let f = Fixture::new();
    producer(&f, true, false);
    let report = run_unchanged(&f, &evidence_run(&f));
    assert!(!report.passed());
    assert!(
        report
            .samples()
            .iter()
            .all(|s| s.decision == Decision::Pass)
    );
    let evidence = report.residual_evidence().unwrap();
    assert!(matches!(
        evidence.members[0].artifact,
        ArtifactOutcome::Unavailable { .. }
    ));
    assert!(matches!(
        evidence.pools[0],
        ProfileEvidence::Unavailable { .. }
    ));
}

#[test]
fn answer_capture_has_first_claim_on_the_slot_allowance() {
    let f = Fixture::new();
    let ordinary = producer(&f, true, true);
    let mut request = evidence_run(&f);
    request.limits.process.max_output_bytes = ordinary + 128;
    let report = run_unchanged(&f, &request);
    assert!(
        report
            .samples()
            .iter()
            .all(|s| s.decision == Decision::Pass)
    );
    assert!(
        matches!(&report.residual_evidence().unwrap().members[0].artifact,
        ArtifactOutcome::Unavailable{detail} if detail.contains("allowance"))
    );
    assert!(!report.passed());
}

#[test]
fn changed_artifact_prevents_manifest_publication() {
    let f = Fixture::new();
    producer(&f, true, true);
    let report = run_unchanged(&f, &evidence_run(&f));
    assert!(report.passed(), "{:#?}", report.residual_evidence());
    let ArtifactOutcome::Retained { path, .. } =
        &report.residual_evidence().unwrap().members[0].artifact
    else {
        panic!("retained artifact")
    };
    let artifact = f.report.parent().unwrap().join(path);
    let mut bytes = fs::read(&artifact).unwrap();
    bytes[0] ^= 1;
    fs::write(&artifact, bytes).unwrap();
    assert!(report.publish().is_err());
    assert!(!f.report.exists());
    drop(report);
    assert!(!artifact.exists());
}

#[test]
fn an_unavailable_member_withholds_the_whole_profile_pool() {
    let f = Fixture::new();
    producer(&f, true, true);
    let once = quote(f.directory.path().join("first-artifact").to_str().unwrap());
    let script = fs::read_to_string(&f.native).unwrap();
    fs::write(
        &f.native,
        script.replace(
            "if [ -n \"$request\" ]; then",
            &format!("if [ -n \"$request\" ] && mkdir {once} 2>/dev/null; then"),
        ),
    )
    .unwrap();
    let report = crate::performance::matrix::run_workloads(
        &evidence_run(&f),
        &[f.unchanged(), f.workload.clone()],
    )
    .unwrap();
    assert!(
        report
            .samples()
            .iter()
            .all(|s| s.decision == Decision::Pass)
    );
    let evidence = report.residual_evidence().unwrap();
    assert!(matches!(
        evidence.members[0].artifact,
        ArtifactOutcome::Retained { .. }
    ));
    assert!(matches!(
        evidence.members[1].artifact,
        ArtifactOutcome::Unavailable { .. }
    ));
    assert!(matches!(
        evidence.pools[0],
        ProfileEvidence::Unavailable { .. }
    ));
    assert!(!report.passed());
}
