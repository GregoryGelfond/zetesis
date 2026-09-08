use super::{Case, Configuration};
use serde::{Serialize, Serializer, ser::SerializeStruct};
use zetesis_wgpu::{AggregateGpuActivity, AggregateGpuBatchStats};

/// An actual reduction route, independent of the requested physical API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// Native ordered scalar reductions.
    Scalar,
    /// The same native reductions in the owned indexed Rayon pool.
    Rayon,
    /// Dedicated device whose group/transport is dropped before every sample.
    DeviceFresh,
    /// Independent device retaining this case's group and exact transport.
    DeviceResident,
}

/// Disjoint observations; initial is not a process-cold cache claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// First route observation on this case, including resident priming.
    Initial,
    /// Recorded preparation observation outside the timed population.
    Warmup,
    /// Requested rotated repeated sample.
    Timed,
}

/// Exact measure view shared by scalar, Rayon and numeric device results.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum Value {
    /// Mathematical integer retained without narrowing the native CPU result.
    Integer(i128),
    /// Genuine empty-maximum endpoint.
    Infimum,
    /// Genuine empty-minimum endpoint.
    Supremum,
}

/// One measure and the conjunction of its retained guards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Evaluation {
    /// Complete numeric measure or empty endpoint.
    pub value: Value,
    /// All retained guards hold of this measure.
    pub holds: bool,
}

/// Ordered original/frozen aggregate result; no stable-membership assertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// Actual original measure and guard truth.
    pub original: Evaluation,
    /// Actual frozen measure and guards, absent when not acquired.
    pub frozen: Option<Evaluation>,
    /// Original guard truth conjoined with frozen guard truth, if acquired.
    pub reduct_truth: Option<bool>,
}

/// Unmodified native device activity plus success-only batch accounting.
#[derive(Clone, Copy, Debug)]
pub struct DeviceWork {
    /// Actual submitted work and complete-batch validated readback accounting.
    pub activity: AggregateGpuActivity,
    /// Successful cache/active-payload accounting; never process RSS.
    pub batch: Option<AggregateGpuBatchStats>,
}
impl Serialize for DeviceWork {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("DeviceWork", 8)?;
        state.serialize_field("submissions", &self.activity.submissions)?;
        state.serialize_field(
            "submitted_occurrences",
            &self.activity.submitted_occurrences,
        )?;
        state.serialize_field("scheduled_work", &self.activity.scheduled_work)?;
        state.serialize_field(
            "completed_occurrences",
            &self.activity.completed_occurrences,
        )?;
        state.serialize_field("completed_work", &self.activity.completed_work)?;
        state.serialize_field("uploaded_bytes", &self.activity.uploaded_bytes)?;
        state.serialize_field("downloaded_bytes", &self.activity.downloaded_bytes)?;
        state.serialize_field("batch", &self.batch.as_ref().map(Batch))?;
        state.end()
    }
}
struct Batch<'a>(&'a AggregateGpuBatchStats);
impl Serialize for Batch<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = self.0;
        let mut state = serializer.serialize_struct("AggregateGpuBatchStats", 9)?;
        state.serialize_field("group_uploaded", &value.group_uploaded)?;
        state.serialize_field("transport_allocated", &value.transport_allocated)?;
        state.serialize_field("incoming_resident_bytes", &value.incoming_resident_bytes)?;
        state.serialize_field("resident_group_bytes", &value.resident_group_bytes)?;
        state.serialize_field("resident_transport_bytes", &value.resident_transport_bytes)?;
        state.serialize_field("accounted_bytes", &value.accounted_bytes)?;
        state.serialize_field("host_work", &value.host_work)?;
        state.serialize_field("occurrences", &value.occurrences)?;
        state.serialize_field("device_work", &value.device_work)?;
        state.end()
    }
}

/// Attempt accounting retained even when an operation cannot publish a sample.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Activity {
    /// Scalar/Rayon reductions invoked, including errors from joined tasks.
    pub cpu_attempts: usize,
    /// Successfully completed joined reductions; no partial sample is accepted.
    pub cpu_completed: usize,
    /// Actual charged native reduction work, including failed prefixes.
    pub cpu_work: u64,
    /// Actual device accounting, present only for device routes.
    pub device: Option<DeviceWork>,
}

/// Measured route position and actual attempt accounting.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Observation {
    /// Configured case ordinal; duplicate cases remain separate.
    pub case_index: usize,
    /// Disjoint first, warmup or repeated population.
    pub phase: Phase,
    /// Zero-based index within that population.
    pub iteration: usize,
    /// Position in the rotated route order.
    pub position: usize,
    /// Route actually invoked.
    pub route: Route,
    /// Whole host call including result allocation, conversion and device wait.
    pub elapsed_ns: u128,
    /// Actual attempted/completed work, also retained on failed observations.
    pub activity: Activity,
}

/// Successful sample after exact ordered reference equality outside the timer.
#[derive(Clone, Debug, Serialize)]
pub struct Sample {
    /// Actual route and work.
    pub observation: Observation,
    /// Every original/frozen occurrence, including repetitions.
    pub outcomes: Vec<Outcome>,
}

/// Separately timed preparation; none of these intervals is reduction time.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Preparation {
    /// Native Group/Theory and fixed interpretation population construction.
    pub fixture_ns: u128,
    /// Actual original/frozen formula acquisition and mask allocation.
    pub acquisition_ns: u128,
    /// Numeric wire preparation and signed carrier validation.
    pub numeric_ns: u128,
    /// Native exact reference reductions and output-view allocation.
    pub reference_ns: u128,
    /// Charged native Group admission work.
    pub admission_work: u64,
    /// Charged actual formula/tuple eligibility work across all occurrences.
    pub acquisition_work: u64,
    /// Sum of retained eligibility-mask payload; excludes borrowed Group/M/J.
    pub eligibility_bytes: u64,
    /// Largest single acquisition's authored simultaneous mask/scratch payload.
    pub acquisition_peak_bytes: u64,
    /// Checked numeric tuple/guard preparation work.
    pub numeric_work: u64,
    /// Retained prepared numeric wire payload, not driver memory.
    pub numeric_bytes: u64,
    /// Charged native exact reference reduction work.
    pub reference_work: u64,
}

/// Synchronous experiment view; every event is emitted outside sample clocks.
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event<'a> {
    /// Complete finite configuration before resource creation.
    Configuration {
        /// Version of this event schema.
        schema: u32,
        /// Requested physical API, schedule and independent operation bounds.
        configuration: &'a Configuration,
        /// Exact included/excluded measurement boundaries.
        scope: &'static str,
    },
    /// One successfully initialized owned pool or device/pipeline.
    Setup {
        /// Route owning the resource.
        route: Route,
        /// Initialization outside all reduction clocks.
        elapsed_ns: u128,
        /// Actual hardware metadata; absent for Rayon. This reusable metadata
        /// view is shared with the existing tight experiment.
        adapter: Option<crate::tight_measurement::Device<'a>>,
    },
    /// Complete acquired observations and reference, before samples.
    Prepared {
        /// Configured ordinal.
        case_index: usize,
        /// Complete deterministic group dimensions.
        case: Case,
        /// Versioned deterministic input grammar, documented beside the command.
        fixture_version: u32,
        /// Separate host preparation costs and authored payload.
        preparation: Preparation,
        /// Per-occurrence M/J atom bit patterns; None means original-only.
        interpretations: &'a [(u8, Option<u8>)],
        /// Every exact native measure/guard reference, in occurrence order.
        reference: &'a [Outcome],
    },
    /// One completed and independently compared sample.
    Sample {
        /// Actual observation and complete ordered values.
        sample: &'a Sample,
    },
    /// Measured attempt failed; no replacement position or completion follows.
    Failed {
        /// Actual elapsed time and retained work prefix.
        observation: Observation,
        /// Human-readable typed failure.
        reason: &'a str,
    },
    /// Every requested initial/warmup/timed observation completed.
    Complete {
        /// Total committed samples, including initialization and warmups.
        samples: usize,
    },
}
