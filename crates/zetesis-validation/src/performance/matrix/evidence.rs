//! Optional, untimed evidence for a caller-selected family of GPU fixtures.
//! The ordinary answer capture always has first claim on its byte bounds.
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{Decision, DeviceWork, Producer, Report, Request, Sample, Slot};
use crate::residual_evidence as wire;

/// Protocol requested explicitly in the child process's extended help.
const REQUEST_ARGUMENT: &str = "--residual-evidence-request";

/// Membership of a supplementary campaign and the independent result of acquiring its samples.
#[derive(Serialize)]
pub struct ResidualEvidence {
    /// Explicit fixture family. Each native profile forms a separate pool.
    pub cohort: String,
    /// One entry per requested native case/profile, in case-major order.
    pub members: Vec<ResidualEvidenceMember>,
    /// Pooled measurements, or a reason they could not be obtained, per profile.
    pub pools: Vec<ProfileEvidence>,
    #[serde(skip)]
    storage: Option<Store>,
    #[serde(skip)]
    accumulating: Vec<Option<Result<wire::Pool, String>>>,
}
impl std::fmt::Debug for ResidualEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResidualEvidence")
            .field("cohort", &self.cohort)
            .field("members", &self.members)
            .field("pools", &self.pools)
            .finish_non_exhaustive()
    }
}

/// One member refers to the case, executable seals and requested profile of the same report.
#[derive(Debug, Serialize)]
pub struct ResidualEvidenceMember {
    /// Index in the campaign's sealed case order.
    pub case: usize,
    /// Index in the requested profiles.
    pub profile: usize,
    /// Artifact status independent of the child's answer decision.
    pub artifact: ArtifactOutcome,
}

/// An artifact's byte identity or the reason it is unavailable.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ArtifactOutcome {
    /// The corresponding native child never reached artifact collection.
    Unattempted,
    /// A requested measurement could not be used.
    Unavailable {
        /// Negotiation, qualification, capture or publication failure.
        detail: String,
    },
    /// Complete, validated bytes retained under the capture allowances.
    Retained {
        /// Relative to the published report's directory.
        path: PathBuf,
        /// Request identity copied into the validated header.
        nonce: [u8; 16],
        /// Full file length.
        bytes: u64,
        /// SHA-256 of the validated bytes.
        sha256: String,
    },
}

/// A complete profile pool, or the reason the fixture population was not pooled.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProfileEvidence {
    /// Includes every selected fixture for this profile.
    Complete {
        /// Pooled results and sampling provenance.
        measurements: Box<wire::PooledSamples>,
    },
    /// A profile with missing evidence has no whole-family ranks.
    Unavailable {
        /// The missing requirement.
        detail: String,
    },
}

#[derive(Debug)]
struct Store {
    directory: Option<tempfile::TempDir>,
    published: AtomicBool,
}

impl Drop for Store {
    fn drop(&mut self) {
        // Unpublished reports own their artifacts. Once publication succeeds,
        // the files belong to the report and survive its in-memory lifetime.
        if self.published.load(Ordering::Relaxed)
            && let Some(dir) = self.directory.take()
        {
            let _ = dir.keep();
        }
    }
}

impl Store {
    fn directory(&self) -> &Path {
        self.directory
            .as_ref()
            .expect("live evidence directory")
            .path()
    }
}

fn private_directory(parent: &Path, prefix: &str) -> io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(prefix).rand_bytes(16);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    builder.tempdir_in(parent)
}

/// The request directory lives through one solve and is removed before the next child starts.
pub(super) struct Offer {
    directory: tempfile::TempDir,
    request: wire::Request,
}

impl ResidualEvidence {
    pub(super) fn new(cohort: String, cases: usize, profiles: usize) -> Self {
        Self {
            cohort,
            members: (0..cases)
                .flat_map(|case| {
                    (0..profiles).map(move |profile| ResidualEvidenceMember {
                        case,
                        profile,
                        artifact: ArtifactOutcome::Unattempted,
                    })
                })
                .collect(),
            pools: (0..profiles)
                .map(|_| ProfileEvidence::Unavailable {
                    detail: "incomplete fixture population".into(),
                })
                .collect(),
            accumulating: (0..profiles).map(|_| None).collect(),
            storage: None,
        }
    }

    fn member(&mut self, case: usize, profile: usize) -> &mut ArtifactOutcome {
        &mut self.members[case * self.pools.len() + profile].artifact
    }

    fn unavailable(&mut self, case: usize, profile: usize, detail: String) {
        *self.member(case, profile) = ArtifactOutcome::Unavailable { detail };
    }

    fn store(&mut self, report: &Path) -> io::Result<&Store> {
        if self.storage.is_none() {
            let parent = report
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let directory =
                private_directory(&std::fs::canonicalize(parent)?, "zetesis-residual-")?;
            self.storage = Some(Store {
                directory: Some(directory),
                published: AtomicBool::new(false),
            });
        }
        Ok(self.storage.as_ref().expect("store admitted"))
    }

    fn offer(
        &mut self,
        report: &Path,
        slot: Slot,
        profile: usize,
        available: usize,
    ) -> io::Result<Offer> {
        let store = self.store(report)?;
        let directory = private_directory(store.directory(), "request-")?;
        let mut digest = Sha256::new();
        digest.update(directory.path().as_os_str().as_encoded_bytes());
        digest.update((slot.case as u64).to_le_bytes());
        digest.update((profile as u64).to_le_bytes());
        let hash = digest.finalize();
        let nonce = hash[..16].try_into().expect("SHA-256 supplies 16 bytes");
        let request = wire::Request {
            version: 1,
            nonce,
            max_bytes: wire::MAX_BYTES.min(available as u64),
        };
        request.write(&directory.path().join(wire::REQUEST_NAME))?;
        Ok(Offer { directory, request })
    }

    pub(super) fn passed(&self) -> bool {
        self.members
            .iter()
            .all(|m| matches!(m.artifact, ArtifactOutcome::Retained { .. }))
            && self
                .pools
                .iter()
                .all(|p| matches!(p, ProfileEvidence::Complete { .. }))
    }

    pub(super) fn published(&self) {
        if let Some(store) = &self.storage
            && self
                .members
                .iter()
                .any(|m| matches!(m.artifact, ArtifactOutcome::Retained { .. }))
        {
            store.published.store(true, Ordering::Relaxed);
        }
    }

    /// Recheck retained bytes immediately before publishing their manifest.
    /// As for the ordinary report the caller owns the destination directory.
    pub(super) fn recheck(&self) -> Result<(), crate::selected::Error> {
        let Some(store) = &self.storage else {
            return Ok(());
        };
        let directory = store.directory();
        let parent = directory.parent().expect("private sibling directory");
        let canonical = std::fs::canonicalize(directory)
            .map_err(|e| crate::selected::identity::io(directory, e))?;
        if canonical != directory {
            return Err(crate::selected::Error::Path {
                path: directory.to_owned(),
                detail: "residual artifact directory changed",
            });
        }
        for member in &self.members {
            if let ArtifactOutcome::Retained {
                path,
                bytes,
                sha256,
                ..
            } = &member.artifact
            {
                let path = parent.join(path);
                if path.parent() != Some(directory) {
                    return Err(crate::selected::Error::Path {
                        path,
                        detail: "residual artifact escapes its publication directory",
                    });
                }
                wire::verify_digest(&path, *bytes, sha256)
                    .map_err(|e| crate::selected::identity::io(&path, e))?;
            }
        }
        Ok(())
    }

    pub(super) fn finish(&mut self, unchanged: bool) {
        for (index, pool) in self.accumulating.iter_mut().enumerate() {
            let Some(pool) = pool.take() else { continue };
            if !unchanged
                || self
                    .members
                    .iter()
                    .filter(|m| m.profile == index)
                    .any(|m| !matches!(m.artifact, ArtifactOutcome::Retained { .. }))
            {
                continue;
            }
            self.pools[index] = match pool.and_then(|pool| pool.finish().map_err(|e| e.to_string()))
            {
                Ok(result) => ProfileEvidence::Complete {
                    measurements: Box::new(result),
                },
                Err(detail) => ProfileEvidence::Unavailable { detail },
            };
        }
    }
}

/// Negotiate against the frozen child's retained help. An older child still
/// runs its ordinary qualification and receives no unrecognized option.
pub(super) fn prepare(
    request: &Request<'_>,
    report: &mut Report,
    slot: Slot,
    args: &mut Vec<OsString>,
) -> Option<Offer> {
    let Producer::Native { profile } = slot.producer else {
        return None;
    };
    let evidence = report.residual_evidence.as_mut()?;
    let supported = report.metadata.get(1).is_some_and(|help| {
        help.0.stdout().split(|&b| b == b'\n').any(|line| {
            // The two maintained invocation surfaces print the option and
            // its operand on a declaration line, then explain it below.
            line.split(u8::is_ascii_whitespace)
                .filter(|s| !s.is_empty())
                .eq([REQUEST_ARGUMENT.as_bytes(), b"<FILE>"])
        })
    });
    if !supported {
        evidence.unavailable(
            slot.case,
            profile,
            "executable does not advertise residual evidence".into(),
        );
        return None;
    }
    let available = report
        .limits
        .max_total_capture_bytes
        .saturating_sub(report.total_capture_bytes)
        .min(report.limits.process.max_output_bytes);
    match evidence.offer(request.report, slot, profile, available) {
        Ok(offer) => {
            // Options precede the source; the source remains the final argument.
            let source = args.pop().expect("native invocation includes the entry");
            args.extend([
                REQUEST_ARGUMENT.into(),
                offer
                    .directory
                    .path()
                    .join(wire::REQUEST_NAME)
                    .into_os_string(),
                source,
            ]);
            Some(offer)
        }
        Err(error) => {
            evidence.unavailable(slot.case, profile, error.to_string());
            None
        }
    }
}

pub(super) fn collect(report: &mut Report, sample: &Sample, offer: Option<Offer>) {
    let Some(offer) = offer else { return };
    let Producer::Native { profile } = sample.slot.producer else {
        return;
    };
    let capture = sample
        .capture()
        .expect("only a launched child is collected");
    let ordinary = capture.stdout().len() + capture.stderr().len();
    let available = report
        .limits
        .process
        .max_output_bytes
        .saturating_sub(ordinary)
        .min(
            report
                .limits
                .max_total_capture_bytes
                .saturating_sub(report.total_capture_bytes),
        );
    let evidence = report
        .residual_evidence
        .as_mut()
        .expect("requested evidence");
    match retain(evidence, sample, &offer, profile, available) {
        Ok((entry, bytes)) => {
            *evidence.member(sample.slot.case, profile) = entry;
            report.total_capture_bytes += bytes;
        }
        Err(detail) => evidence.unavailable(sample.slot.case, profile, detail),
    }
}

/// Link the child's complete file without replacement, authenticate the linked
/// bytes, then charge them. The final link is removed on any verification failure.
fn retain(
    evidence: &mut ResidualEvidence,
    sample: &Sample,
    offer: &Offer,
    profile: usize,
    available: usize,
) -> Result<(ArtifactOutcome, usize), String> {
    if sample.decision != Decision::Pass {
        return Err("solver did not qualify the complete family".into());
    }
    let Some(DeviceWork::Formula {
        gpu_residual_handovers: Some(handovers),
        ..
    }) = sample.observation.as_ref().map(|o| &o.execution.device)
    else {
        return Err("qualified route lacks general GPU handover observations".into());
    };
    let summary = handovers
        .samples
        .as_deref()
        .ok_or("run did not publish its sample summary")?;
    let source = offer.directory.path().join(wire::ARTIFACT_NAME);
    let metadata = std::fs::symlink_metadata(&source).map_err(|e| e.to_string())?;
    let bytes = usize::try_from(metadata.len()).map_err(|e| e.to_string())?;
    if !metadata.file_type().is_file()
        || bytes > available
        || metadata.len() > offer.request.max_bytes
    {
        return Err("artifact is not a regular file within the remaining capture allowance".into());
    }
    let dir = evidence
        .storage
        .as_ref()
        .expect("allocated for offer")
        .directory();
    let name = format!("case-{}-profile-{profile}.bin", sample.slot.case);
    let destination = dir.join(&name);
    let relative = Path::new(dir.file_name().expect("private sibling directory")).join(name);
    std::fs::hard_link(&source, &destination).map_err(|e| e.to_string())?;
    let checked = wire::read(&destination, &offer.request, summary, true).and_then(|artifact| {
        if artifact.byte_len() != metadata.len() {
            return Err(io::Error::other("artifact length changed"));
        }
        let sha256 = artifact.digest().to_owned();
        let accumulator = evidence.accumulating[profile].get_or_insert_with(|| {
            Ok(wire::Pool::new(u64::from_le_bytes(
                offer.request.nonce[..8].try_into().expect("nonce prefix"),
            )))
        });
        if let Ok(pool) = accumulator
            && let Err(error) = pool.push(artifact)
        {
            // The file remains valid evidence for its own invocation;
            // the pool refuses publication if independence or coverage failed.
            *accumulator = Err(error.to_string());
        }
        Ok(sha256)
    });
    match checked {
        Ok(sha256) => Ok((
            ArtifactOutcome::Retained {
                path: relative,
                nonce: offer.request.nonce,
                bytes: metadata.len(),
                sha256,
            },
            bytes,
        )),
        Err(error) => {
            std::fs::remove_file(destination)
                .map_err(|cleanup| format!("{error}; artifact cleanup: {cleanup}"))?;
            Err(error.to_string())
        }
    }
}
