use serde::{Serialize, Serializer};
use zetesis_ferraris::{Node, TightProducerKind, TightVerdict};

use super::{Case, Configuration};

/// Actual certificate route; all residual completions are serial CPU queries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// Scalar certificate classification.
    Scalar,
    /// Indexed classification in the owned Rayon pool.
    Rayon,
    /// Dedicated GPU instance cleared before every sample; clearing is untimed.
    MetalFresh,
    /// A separate GPU instance retained across this case's observations.
    MetalResident,
    /// Dedicated Vulkan instance cleared before every sample; clearing is untimed.
    VulkanFresh,
    /// A separate Vulkan instance retained across this case's observations.
    VulkanResident,
}

/// Disjoint sample populations; initial is not a process-cold cache claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// First observation of a route on a case, including resident priming.
    Initial,
    /// Retained preparation observation.
    Warmup,
    /// Requested repeated measurement.
    Timed,
}

/// Certificate result with its exact original-root or unsupported-atom witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "kebab-case")]
pub enum Certificate {
    /// Ranked support establishes stable-model membership.
    Stable,
    /// Original satisfaction failed at this asserted node.
    NotModel {
        /// Original root node identity, not its list position.
        root: usize,
    },
    /// Exact CPU reduct completion is required.
    Residual {
        /// First unsupported present atom in ascending order.
        unsupported_atom: usize,
    },
}
impl From<TightVerdict> for Certificate {
    fn from(verdict: TightVerdict) -> Self {
        match verdict {
            TightVerdict::Stable => Self::Stable,
            TightVerdict::NotModel { root } => Self::NotModel { root },
            TightVerdict::Residual { unsupported_atom } => Self::Residual { unsupported_atom },
        }
    }
}

/// Complete membership data, not a forgeable native stable-interpretation receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "kebab-case")]
pub enum Decision {
    /// Complete stable membership.
    Stable,
    /// The candidate does not satisfy the original theory.
    NotModel,
    /// A validated proper subset satisfies the candidate's frozen reduct.
    NonMinimal {
        /// Ascending semantic atom identities of the actual returned witness.
        witness: Vec<usize>,
    },
    /// The complete tight certificate refuted the candidate by the support
    /// law: this present atom has no producer with a true body.
    Unsupported {
        /// The first unsupported present atom.
        atom: usize,
    },
}

/// One occurrence's certificate and complete decision, in original input order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// Actual classification, retaining exact witness identity.
    pub certificate: Certificate,
    /// Actual final status and any validated reduct witness.
    pub decision: Decision,
}

/// Device attempt accounting. Scheduled work is not claimed completed work.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct DeviceWork {
    /// Real queue submissions, including any subsequently failed attempt.
    pub submissions: u64,
    /// Candidate occurrences associated with those submissions.
    pub submitted_candidates: u64,
    /// Fixed shader schedule of the submitted batch.
    pub scheduled_work: u64,
    /// Successfully validated readback occurrences.
    pub completed_candidates: u64,
    /// Work reported by validated readback; not a committed membership result.
    pub completed_work: u64,
    /// Authored initialized/write-buffer payload, not measured bus traffic.
    pub uploaded_bytes: u64,
    /// Completed authored readback payload, not measured bus traffic.
    pub downloaded_bytes: u64,
    /// Success-only cache/storage accounting; absent after any failed call.
    pub residency: Option<Residency>,
}

/// Successful logical GPU allocation accounting; none of these fields is RSS.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Residency {
    /// Whether this call uploaded the complete theory.
    pub theory_uploaded: bool,
    /// Whether this call allocated candidate transport.
    pub transport_allocated: bool,
    /// Retained theory payload bytes.
    pub theory_bytes: u64,
    /// Retained candidate transport payload bytes.
    pub transport_bytes: u64,
    /// Complete authored per-call byte accounting.
    pub accounted_bytes: u64,
    /// Success-only batch submissions, independently cross-checked with activity.
    pub dispatches: u64,
    /// Success-only checked occurrences.
    pub candidates: u64,
    /// Success-only complete shader work.
    pub work: u64,
}

/// Attempt counts retained on success and failure; CPU work excludes GPU work.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Activity {
    /// Scalar or Rayon classification attempts, including every joined error.
    pub cpu_certificate_attempts: usize,
    /// Charged CPU certificate operations actually attempted.
    pub cpu_certificate_work: u64,
    /// Largest successful per-candidate logical certificate payload; not RSS.
    pub cpu_certificate_max_bytes: Option<u64>,
    /// Successfully classified occurrences before any failure.
    pub classified: usize,
    /// Direct stable/nonmodel decisions attributed to a CPU certificate.
    pub cpu_decisions: usize,
    /// Direct stable/nonmodel decisions attributed to GPU classification.
    pub gpu_decisions: usize,
    /// Serial exact CPU queries requested by residual classifications.
    pub residual_attempts: usize,
    /// Those exact queries which completed.
    pub residual_completed: usize,
    /// Actual physical attempt accounting, absent on CPU routes.
    pub device: Option<DeviceWork>,
}

/// Route identity and nested host intervals, before parity/witness validation.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Observation {
    /// Ordinal of the exact configured case, preserving duplicate cases.
    pub case_index: usize,
    /// Disjoint first/warm/timed population.
    pub phase: Phase,
    /// Zero-based iteration within that population.
    pub iteration: usize,
    /// Position within the balanced rotated route order.
    pub position: usize,
    /// Certificate route actually attempted.
    pub route: Route,
    /// Whole call including result allocation and both nested phases.
    pub elapsed_ns: u128,
    /// Directly measured classification call, including device wait/readback.
    pub classification_ns: u128,
    /// Directly measured completion call; absent if classification failed.
    pub completion_ns: Option<u128>,
    /// Actual operation counts, retaining stopped attempts.
    pub activity: Activity,
}

/// A complete, independently validated observation and ordered result vector.
#[derive(Clone, Debug, Serialize)]
pub struct Sample {
    /// Actual measured route and operation counts.
    pub observation: Observation,
    /// Every unfiltered occurrence, including duplicate candidates.
    pub outcomes: Vec<Outcome>,
}

/// Original DAG record, preserving connective and exact operand order.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Formula {
    /// Semantic atom identity.
    Atom(usize),
    /// Falsum.
    False,
    /// Ordered conjunction operands.
    And(usize, usize),
    /// Ordered disjunction operands.
    Or(usize, usize),
    /// Ordered antecedent and consequent.
    Implies(usize, usize),
}
impl From<Node> for Formula {
    fn from(node: Node) -> Self {
        match node {
            Node::Atom(atom) => Self::Atom(atom),
            Node::False => Self::False,
            Node::And(a, b) => Self::And(a, b),
            Node::Or(a, b) => Self::Or(a, b),
            Node::Implies(a, b) => Self::Implies(a, b),
        }
    }
}

/// Producer provenance in complete original-root order.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Producer {
    /// Semantic head atom.
    pub head: usize,
    /// Original body node; absence denotes an unconditional producer.
    pub body: Option<usize>,
    /// Original asserted root node.
    pub root: usize,
    /// Original head form.
    pub kind: ProducerKind,
}

/// Original producer form, not an inferred successful decision category.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProducerKind {
    /// Atomic normal head.
    Normal,
    /// Original atomic choice head.
    Choice,
}
impl From<TightProducerKind> for ProducerKind {
    fn from(kind: TightProducerKind) -> Self {
        match kind {
            TightProducerKind::Normal => Self::Normal,
            TightProducerKind::Choice => Self::Choice,
        }
    }
}

/// Reported adapter identity; metadata equality does not identify a unique chip.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Device<'a> {
    /// Actual reported name.
    pub name: &'a str,
    /// Actual compute API.
    pub backend: &'static str,
    /// Reported device category.
    pub category: &'static str,
    /// Unmodified vendor ID; zero may mean unknown.
    pub vendor_id: u32,
    /// Unmodified device ID; not a unique physical identity.
    pub device_id: u32,
    /// Optional reported bus identity.
    pub pci_bus_id: Option<&'a str>,
    /// Optional reported driver name.
    pub driver: Option<&'a str>,
    /// Optional reported driver version/details.
    pub driver_info: Option<&'a str>,
}
impl<'a> From<zetesis_wgpu::AdapterMetadata<'a>> for Device<'a> {
    fn from(metadata: zetesis_wgpu::AdapterMetadata<'a>) -> Self {
        Self {
            name: metadata.name,
            backend: metadata.backend.label(),
            category: metadata.category.label(),
            vendor_id: metadata.vendor_id,
            device_id: metadata.device_id,
            pci_bus_id: metadata.pci_bus_id,
            driver: metadata.driver,
            driver_info: metadata.driver_info,
        }
    }
}

/// Synchronous library observation; an output failure preserves its prefix.
#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event<'a> {
    /// Complete configuration before pool/device setup.
    Configuration {
        /// Version of this event view.
        schema: u32,
        /// Requested operation bounds and finite schedule.
        configuration: &'a Configuration,
        /// Explicit included/excluded timing scopes.
        scope: &'static str,
    },
    /// One successfully constructed resource, outside sample clocks.
    Setup {
        /// Pool or dedicated GPU route owning the resource.
        resource: Route,
        /// Host setup duration, not a sample.
        elapsed_ns: u128,
        /// Actual adapter name; absent for Rayon.
        adapter: Option<Device<'a>>,
    },
    /// Complete theory and candidate/reference evidence, before this case's calls.
    Prepared {
        /// Exact configured ordinal.
        case_index: usize,
        /// Authored grammar, dimensions and independent reference instrument.
        case: Case,
        /// Fixture, certificate and complete reference preparation outside samples.
        preparation_ns: u128,
        /// Actual charged certificate construction work.
        plan_work: u64,
        /// Certificate positive dependency edges.
        plan_dependencies: usize,
        /// Conservative certificate construction payload, excluding the theory.
        plan_construction_bytes: u64,
        /// Retained certificate payload, excluding the theory; not process RSS.
        plan_resident_bytes: u64,
        /// Ordered topological original DAG.
        nodes: &'a [Formula],
        /// Original asserted-root order.
        roots: &'a [usize],
        /// Complete original producers.
        producers: &'a [Producer],
        /// Checked per-atom ranks.
        ranks: &'a [usize],
        /// Ascending atom lists, preserving candidate occurrence order.
        candidates: &'a [Vec<usize>],
        /// Independent complete reference and expected exact certificate witnesses.
        reference: &'a [Outcome],
    },
    /// Complete operation after exact result agreement and witness validation.
    Sample {
        /// Successful observation.
        sample: &'a Sample,
    },
    /// Failed measured operation; none of this attempt is substituted or retried.
    Failed {
        /// Measured prefix and actual activity.
        observation: Observation,
        /// Typed native cause rendered for the output view.
        reason: &'a str,
    },
    /// Every requested position completed; setup/reference failures omit this.
    Complete {
        /// Successful observations, including initial and warmup populations.
        samples: usize,
    },
}

#[derive(Serialize)]
struct ConfigurationView<'a> {
    cases: &'a [Case],
    backend: &'static str,
    support: super::Support,
    warmups: usize,
    repetitions: usize,
    workers: usize,
    residual_policy: &'static str,
    #[serde(with = "PlanLimits")]
    plan_limits: &'a zetesis_ferraris::TightPlanLimits,
    #[serde(with = "CertificateLimits")]
    certificate_limits: &'a zetesis_ferraris::TightCheckLimits,
    #[serde(with = "ReferenceLimits")]
    reference_limits: &'a zetesis_ferraris::Limits,
    #[serde(with = "ResidualLimits")]
    residual_limits: &'a zetesis_sat::Limits,
    #[serde(with = "GpuLimits")]
    gpu_limits: &'a zetesis_wgpu::TightGpuLimits,
}
#[derive(Serialize)]
#[serde(remote = "zetesis_ferraris::TightPlanLimits")]
#[expect(
    clippy::struct_field_names,
    reason = "Remote serialization preserves the native resource-limit vocabulary."
)]
struct PlanLimits {
    max_producers: usize,
    max_dependencies: usize,
    max_bytes: u64,
    max_work: u64,
}
#[derive(Serialize)]
#[serde(remote = "zetesis_ferraris::TightCheckLimits")]
struct CertificateLimits {
    max_bytes: u64,
    max_work: u64,
}
#[derive(Serialize)]
#[serde(remote = "zetesis_ferraris::Limits")]
struct ReferenceLimits {
    max_work: u64,
    max_subsets: u64,
}
#[derive(Serialize)]
#[serde(remote = "zetesis_sat::Limits")]
struct ResidualLimits {
    #[serde(with = "AdmissionLimits")]
    admission: zetesis_sat::AdmissionLimits,
    #[serde(with = "AdmissionLimits")]
    reduct_admission: zetesis_sat::AdmissionLimits,
    #[serde(with = "SearchLimits")]
    search: zetesis_sat::SearchLimits,
    max_candidates: u64,
    max_verification_work: u64,
    max_reduct_bytes: u64,
}
#[derive(Serialize)]
#[serde(remote = "zetesis_sat::AdmissionLimits")]
#[expect(
    clippy::struct_field_names,
    reason = "Remote serialization preserves the native resource-limit vocabulary."
)]
struct AdmissionLimits {
    max_variables: usize,
    max_clauses: usize,
    max_literals: usize,
}
#[derive(Serialize)]
#[serde(remote = "zetesis_sat::SearchLimits")]
struct SearchLimits {
    max_work: u64,
    max_decisions: u64,
}
#[derive(Serialize)]
#[serde(remote = "zetesis_wgpu::TightGpuLimits")]
struct GpuLimits {
    max_candidates: usize,
    max_batch_bytes: u64,
    max_work_per_candidate: u64,
    #[serde(rename = "timeout_ns", serialize_with = "duration")]
    timeout: std::time::Duration,
}
fn duration<S: Serializer>(value: &std::time::Duration, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u128(value.as_nanos())
}
impl Serialize for Configuration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ConfigurationView {
            cases: &self.cases, backend: self.backend.label(),
            support: self.support,
            warmups: self.warmups, repetitions: self.repetitions.get(), workers: self.workers.get(),
            residual_policy: "serial occurrence-ordered native general reduct queries on every route",
            plan_limits: &self.plan_limits, certificate_limits: &self.certificate_limits,
            reference_limits: &self.reference_limits, residual_limits: &self.residual_limits, gpu_limits: &self.gpu_limits,
        }.serialize(serializer)
    }
}
