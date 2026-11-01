use super::*;
use crate::performance::matrix::{
    ResidualQuantiles, ResidualSampleGroup, ResidualSampleGroups, ResidualSampleMetric,
    ResidualSamplePopulation,
};
use serde_json::{Value, json};
use tempfile::TempDir;

fn measure(values: &[u128], population: u64) -> Value {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    // This fixture supplies complete data. Its expected summary is produced
    // independently of the artifact reader and pool.
    assert_eq!(n as u64, population);
    json!({"eligible":population,"known":population,"retained":n,"sum":sorted.iter().sum::<u128>(),
        "maximum":sorted.last(),"median":n.checked_sub(1).map(|last| sorted[last/2].midpoint(sorted[n/2])),
        "p90":n.checked_sub(1).map(|last|sorted[last-n/10]),
        "quantiles":if n==0 {"unavailable"} else {"exact"}})
}

fn batch_metric(n: usize) -> ResidualSampleMetric {
    serde_json::from_value(measure(&vec![0; n], n as u64)).unwrap()
}

fn group(counts: &[u64]) -> ResidualSampleGroup {
    let worlds = usize::try_from(counts.iter().sum::<u64>()).unwrap();
    let b = counts.len();
    let counts: Vec<_> = counts.iter().copied().map(u128::from).collect();
    let empty = || batch_metric(0);
    ResidualSampleGroup {
        open_atoms: batch_metric(worlds),
        visited: empty(),
        attempted_splits: empty(),
        published_splits: empty(),
        depth: empty(),
        work: empty(),
        query_ns: empty(),
        residuals: serde_json::from_value(measure(&counts, b as u64)).unwrap(),
        width: batch_metric(b),
        batch_ns: batch_metric(b),
    }
}

/// All worlds are fixed-point residuals certified on the host. The per-batch
/// residual count can differ across fixtures even when query work is zero.
fn fixture(dir: &Path, nonce: u8, counts: &[u64]) -> (Request, ResidualRunSamples, Vec<u8>) {
    let request = Request {
        version: 1,
        nonce: [nonce; 16],
        max_bytes: MAX_BYTES,
    };
    let n = counts.iter().sum::<u64>();
    let b = counts.len();
    let pool = |n: usize| ResidualSamplePopulation {
        seen: Some(n as u64),
        retained: n,
        capacity: n,
    };
    let groups = ResidualSampleGroups {
        all: group(counts),
        fixed_point: group(counts),
        round_limit: group(&vec![0; b]),
        work_limit: group(&vec![0; b]),
    };
    let summary = ResidualRunSamples {
        version: 1,
        algorithm: "splitmix64-reject-v1".into(),
        seed: u64::from(nonce),
        byte_limit: 4096,
        retained_bytes: 0,
        peak_bytes: 1024,
        worlds: pool(usize::try_from(n).unwrap()),
        batches: pool(b),
        groups: Some(Box::new(groups)),
    };
    let h = Header {
        nonce: request.nonce,
        seed: summary.seed,
        worlds: Some(n),
        batches: Some(b as u64),
        retained_worlds: u32::try_from(n).unwrap(),
        retained_batches: u32::try_from(b).unwrap(),
        complete: true,
    };
    let mut bytes = h.encode().to_vec();
    for _ in 0..n {
        bytes.extend(
            World {
                reason: Some(zetesis_backend::residual_evidence::Reason::FixedPoint),
                open: Some(0),
                query: zetesis_backend::residual_evidence::Query::Certified,
            }
            .encode(),
        );
    }
    for &c in counts {
        bytes.extend(
            Batch {
                residuals: [Some(c), Some(c), Some(0), Some(0)],
                width: Some(0),
                host_ns: [Some(0); 4],
            }
            .encode(),
        );
    }
    std::fs::write(dir.join(ARTIFACT_NAME), &bytes).unwrap();
    (request, summary, bytes)
}

#[test]
fn exact_fixture_pool_weights_each_committed_batch_once() {
    let dir = TempDir::new().unwrap();
    let mut pool = Pool::new(19);
    for (nonce, counts) in [(1, &[9][..]), (2, &[0, 1][..])] {
        let (request, summary, bytes) = fixture(dir.path(), nonce, counts);
        let artifact = read(&dir.path().join(ARTIFACT_NAME), &request, &summary, true).unwrap();
        assert_eq!(artifact.digest(), crate::digest::sha256(&bytes));
        assert_eq!(artifact.byte_len(), bytes.len() as u64);
        pool.push(artifact).unwrap();
    }
    let pooled = pool.finish().unwrap();
    assert_eq!(pooled.worlds.seen, Some(10));
    assert_eq!(pooled.batches.seen, Some(3));
    assert_eq!(pooled.groups.all.residuals.median, Some(1));
    assert_eq!(pooled.groups.all.residuals.p90, Some(9));
    assert_eq!(
        pooled.groups.all.residuals.quantiles,
        ResidualQuantiles::Exact
    );
    assert_eq!(pooled.groups.all.depth.eligible, Some(0));
}

#[test]
fn read_binds_record_values_to_same_invocation_summary() {
    let dir = TempDir::new().unwrap();
    let (request, summary, bytes) = fixture(dir.path(), 1, &[1]);
    let file = dir.path().join(ARTIFACT_NAME);
    assert!(
        read(
            &file,
            &Request {
                nonce: [0; 16],
                ..request
            },
            &summary,
            true
        )
        .is_err()
    );
    assert!(read(&file, &request, &summary, false).is_err());
    assert!(
        read(
            &file,
            &Request {
                max_bytes: 128,
                ..request
            },
            &summary,
            true
        )
        .is_err()
    );
    for length in [0, 127, 128, bytes.len() - 1, bytes.len() + 1] {
        let mut malformed = bytes.clone();
        malformed.resize(length, 0);
        std::fs::write(&file, malformed).unwrap();
        assert!(read(&file, &request, &summary, true).is_err());
    }
    let mut mismatched = bytes;
    mismatched[128 + 8] = 1; // Valid world encoding, different open-atom measurement.
    std::fs::write(&file, mismatched).unwrap();
    assert!(read(&file, &request, &summary, true).is_err());
}

#[test]
fn duplicate_or_incomplete_invocations_cannot_yield_a_complete_pool() {
    let dir = TempDir::new().unwrap();
    let (r, s, mut bytes) = fixture(dir.path(), 8, &[0]);
    let f = dir.path().join(ARTIFACT_NAME);
    let mut pool = Pool::new(5);
    pool.push(read(&f, &r, &s, true).unwrap()).unwrap();
    assert!(pool.push(read(&f, &r, &s, true).unwrap()).is_err());
    assert!(pool.finish().is_err());
    bytes[16] &= !1;
    std::fs::write(&f, bytes).unwrap();
    assert!(Pool::new(5).push(read(&f, &r, &s, false).unwrap()).is_err());
}

#[test]
fn request_is_bounded_strict_and_exclusive() {
    let dir = TempDir::new().unwrap();
    let f = dir.path().join(REQUEST_NAME);
    let req = Request {
        version: 1,
        nonce: [42; 16],
        max_bytes: 0,
    };
    req.write(&f).unwrap();
    assert_eq!(Request::read(&f).unwrap(), req);
    assert!(req.write(&f).is_err());
    let mut json = serde_json::to_value(req).unwrap();
    json["unexpected"] = json!(true);
    std::fs::write(&f, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(Request::read(&f).is_err());
    std::fs::write(&f, [b' '; 1025]).unwrap();
    assert!(Request::read(&f).is_err());
}

#[test]
fn publication_rechecks_the_bytes_after_the_child_exits() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join(ARTIFACT_NAME);
    let (r, s, mut bytes) = fixture(dir.path(), 3, &[0]);
    let artifact = read(&path, &r, &s, true).unwrap();
    verify_digest(&path, artifact.byte_len(), artifact.digest()).unwrap();
    bytes[40] ^= 1;
    std::fs::write(&path, &bytes).unwrap();
    assert!(verify_digest(&path, artifact.byte_len(), artifact.digest()).is_err());
    #[cfg(unix)]
    {
        let other = dir.path().join("renamed.bin");
        std::fs::rename(&path, &other).unwrap();
        std::os::unix::fs::symlink(&other, &path).unwrap();
        assert!(verify_digest(&path, bytes.len() as u64, &crate::digest::sha256(&bytes)).is_err());
    }
}

#[test]
fn a_census_distinguishes_certification_from_missing_query_observations() {
    let dir = TempDir::new().unwrap();
    let (request, summary, mut bytes) = fixture(dir.path(), 1, &[1]);
    // Only change the query state: numerical payload remains all zeros.
    bytes[128] &= !(1 << 3);
    let path = dir.path().join(ARTIFACT_NAME);
    std::fs::write(&path, bytes).unwrap();
    assert!(read(&path, &request, &summary, true).is_err());
}

#[test]
fn a_known_population_cannot_be_thinned_before_export() {
    let dir = TempDir::new().unwrap();
    let (request, mut summary, mut bytes) = fixture(dir.path(), 1, &[1]);
    let mut header = Header::decode(bytes[..128].try_into().unwrap()).unwrap();
    header.worlds = Some(2);
    bytes[..128].copy_from_slice(&header.encode());
    summary.worlds.seen = Some(2);
    summary.worlds.capacity = 2;
    summary.groups.as_mut().unwrap().all.residuals.sum = Some(2);
    let path = dir.path().join(ARTIFACT_NAME);
    std::fs::write(&path, bytes).unwrap();
    let error = read(&path, &request, &summary, true).unwrap_err();
    assert!(error.to_string().contains("thinned"));
}

#[test]
fn two_truncated_streams_with_the_same_seed_do_not_qualify_as_independent() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join(ARTIFACT_NAME);
    let mut pool = Pool::new(7);
    for nonce in [1, 2] {
        let (request, mut summary, mut bytes) = fixture(dir.path(), nonce, &[1, 1]);
        let mut header = Header::decode(bytes[..128].try_into().unwrap()).unwrap();
        header.retained_worlds = 1;
        header.seed = 43;
        summary.seed = 43;
        summary.worlds.capacity = 1;
        summary.worlds.retained = 1;
        let groups = summary.groups.as_mut().unwrap();
        for m in [
            &mut groups.all.open_atoms,
            &mut groups.fixed_point.open_atoms,
        ] {
            m.retained = 1;
            m.quantiles = ResidualQuantiles::Estimated;
        }
        bytes[..128].copy_from_slice(&header.encode());
        bytes.drain(200..272);
        std::fs::write(&file, &bytes).unwrap();
        let result = pool.push(read(&file, &request, &summary, true).unwrap());
        assert_eq!(result.is_ok(), nonce == 1);
    }
    assert!(pool.finish().is_err());
}
