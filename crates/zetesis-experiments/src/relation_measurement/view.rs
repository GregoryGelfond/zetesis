use std::io::{self, Write};

use serde::{
    Serialize, Serializer,
    ser::{SerializeSeq, SerializeStruct},
};
use sha2::{Digest, Sha256};
use zetesis_core::{Sign, Value, ValueNode};
use zetesis_wgpu::{RelationGpuActivity, RelationGpuStats};

use super::{
    Configuration, Error,
    config::{family_name, payload_name},
};
use crate::relation_fixtures::Fixture;

const MAX_SUBJECT_BYTES: usize = 64 * 1024 * 1024;

/// Measured execution schedule; all routes publish identical mask layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// Existing scalar relation selection, then ordered mask packing.
    Scalar,
    /// The same selection with disjoint query masks in an owned Rayon pool.
    Rayon,
    /// Physical Metal row tiles and complete readback.
    Metal,
    /// Physical Vulkan row tiles and complete readback.
    Vulkan,
}

/// Initial and warmup populations remain separate from timed repetitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// First observed batch; not a claim of cold hardware caches.
    Initial,
    /// A retained warmup batch.
    Warmup,
    /// A declared timed repetition.
    Timed,
}

/// One-time observed setup intervals and simultaneous authored storage.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Preparation {
    /// Typed source/query fixture construction.
    pub fixture_ns: u128,
    /// Shared typed dictionary and column construction.
    pub relation_ns: u128,
    /// Whole-value keys resolved in that exact dictionary.
    pub query_ns: u128,
    /// Complete original-row input selection construction.
    pub input_ns: u128,
    /// Independently owned Rayon pool construction.
    pub pool_ns: u128,
    /// Real adapter/device/pipeline creation, when requested.
    pub device_ns: Option<u128>,
    /// Column preparation/upload, when requested.
    pub upload_ns: Option<u128>,
    /// Fixture, relation, query and original-input retained authored bytes.
    pub shared_bytes: usize,
    /// Peak fixture/relation/query construction capacity before measurement.
    pub construction_peak_bytes: usize,
    /// Resident device column bytes across repeated calls; zero on CPU-only runs.
    pub column_bytes: u64,
}

/// Actual device work and authored transport, separate from bus traffic or RSS.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct DeviceWork {
    /// Actual queue submissions.
    pub submissions: u64,
    /// Query occurrences submitted.
    pub submitted_queries: u64,
    /// Complete query receipts decoded.
    pub completed_queries: u64,
    /// Actual physical workgroups dispatched.
    pub workgroups: u64,
    /// Scheduled full-scan primitive work.
    pub scheduled_work: u64,
    /// Primitive work with validated complete receipts.
    pub completed_work: u64,
    /// Authored input payload uploaded by this batch.
    pub uploaded_bytes: u64,
    /// Complete readback payload decoded by this batch.
    pub downloaded_bytes: u64,
    /// GPU library's conservative authored allocation sum for this batch.
    pub accounted_bytes: u64,
    /// GPU library's uniform/query/equality/result/readback storage.
    pub transport_bytes: u64,
}

impl DeviceWork {
    pub(super) fn new(activity: RelationGpuActivity, stats: RelationGpuStats) -> Self {
        Self {
            submissions: activity.submissions,
            submitted_queries: activity.submitted_queries,
            completed_queries: activity.completed_queries,
            workgroups: activity.submitted_workgroups,
            scheduled_work: activity.scheduled_work,
            completed_work: activity.completed_work,
            uploaded_bytes: activity.uploaded_bytes,
            downloaded_bytes: activity.downloaded_bytes,
            accounted_bytes: stats.accounted_bytes,
            transport_bytes: stats.transport_bytes,
        }
    }
}

/// One complete matched batch, validated outside its selection interval.
#[derive(Debug, Serialize)]
pub struct Observation<'a> {
    /// Immutable typed source/query identity shared by every route.
    pub subject_sha256: &'a str,
    /// Actual requested execution schedule.
    pub route: Route,
    /// Declared sample population.
    pub phase: Phase,
    /// Zero-based position within this phase.
    pub repetition: usize,
    /// Allocation, equality selection and packing/copy to the common masks.
    pub selection_ns: u128,
    /// Same core mask reconstruction and typed row traversal for every route.
    pub reconstruction_ns: u128,
    /// Sum of those two directly observed intervals; validation/publication excluded.
    pub operation_ns: u128,
    /// Charged core selection work on CPU; zero on device routes.
    pub cpu_selection_work: u128,
    /// Complete typed argument cells visited in reconstruction.
    pub reconstructed_cells: usize,
    /// Inclusive conservative simultaneous authored capacity for this route.
    pub peak_bytes: usize,
    /// Original rows per query.
    pub rows: usize,
    /// Complete original query occurrences.
    pub queries: usize,
    /// Packed words per query in original row order, including zero tail padding.
    pub words_per_query: usize,
    /// Complete query-major packed outputs, preserving repeated queries.
    pub masks: &'a [u32],
    /// Physical evidence, absent for scalar and Rayon routes.
    pub device: Option<DeviceWork>,
}

/// Streaming events; Complete is offered after all prior observations succeed.
/// A failing sink may retain partial event bytes.
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event<'a> {
    /// The finite requested experiment before construction begins.
    Start {
        /// Version of this report schema.
        schema: u32,
        /// Complete explicit requested scope.
        configuration: Configuration,
    },
    /// Exact typed source/query subject and one-time setup evidence.
    Subject {
        /// SHA-256 of the canonical compact serialized subject below.
        sha256: &'a str,
        /// Complete logical source/query content, not dictionary IDs.
        subject: Subject<'a>,
        /// One-time setup and shared authored capacity.
        preparation: Preparation,
        /// Actual physical adapter name, if selected.
        adapter: Option<&'a str>,
    },
    /// One fully checked original query population.
    Observation(Observation<'a>),
    /// Every declared observation was checked and published.
    Complete {
        /// Actual observation count across all routes/phases.
        observations: usize,
    },
}

/// Borrowed typed source view, with canonical field and occurrence order.
pub struct Subject<'a>(pub(super) &'a Fixture);

impl Serialize for Subject<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let specification = self.0.specification();
        let mut state = serializer.serialize_struct("RelationSubject", 8)?;
        state.serialize_field("generator", &specification.version)?;
        state.serialize_field("family", family_name(specification.family))?;
        state.serialize_field("payload", payload_name(specification.payload))?;
        state.serialize_field("predicate", self.0.predicate().name())?;
        state.serialize_field("sign", sign(self.0.predicate().sign()))?;
        state.serialize_field("arity", &self.0.predicate().arity())?;
        state.serialize_field("rows", &Rows(self.0))?;
        state.serialize_field("queries", &Queries(self.0))?;
        state.end()
    }
}

struct Rows<'a>(&'a Fixture);
impl Serialize for Rows<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut rows = serializer.serialize_seq(Some(self.0.atoms().len()))?;
        for atom in self.0.atoms() {
            rows.serialize_element(&Values(atom.values()))?;
        }
        rows.end()
    }
}
struct Queries<'a>(&'a Fixture);
impl Serialize for Queries<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut queries = serializer.serialize_seq(Some(self.0.queries().len()))?;
        for query in self.0.queries() {
            queries.serialize_element(&Keys(query))?;
        }
        queries.end()
    }
}
struct Keys<'a>(&'a [(usize, Value)]);
impl Serialize for Keys<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut keys = serializer.serialize_seq(Some(self.0.len()))?;
        for (column, value) in self.0 {
            keys.serialize_element(&(*column, TypedValue(value)))?;
        }
        keys.end()
    }
}
struct Values<'a>(&'a [Value]);
impl Serialize for Values<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut values = serializer.serialize_seq(Some(self.0.len()))?;
        for value in self.0 {
            values.serialize_element(&TypedValue(value))?;
        }
        values.end()
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
enum ValueView<'a> {
    Infimum,
    Number(i32),
    String(&'a str),
    Symbol(&'a str),
    Structured(Nodes<'a>),
    Supremum,
}
struct TypedValue<'a>(&'a Value);
impl Serialize for TypedValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Infimum => ValueView::Infimum,
            Value::Number(value) => ValueView::Number(*value),
            Value::String(value) => ValueView::String(value),
            Value::Symbol(value) => ValueView::Symbol(value),
            Value::Structured(value) => ValueView::Structured(Nodes(value.nodes())),
            Value::Supremum => ValueView::Supremum,
        }
        .serialize(serializer)
    }
}

struct Nodes<'a>(&'a [ValueNode]);
impl Serialize for Nodes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut nodes = serializer.serialize_seq(Some(self.0.len()))?;
        for node in self.0 {
            let view = match node {
                ValueNode::Infimum => NodeView::Infimum,
                ValueNode::Number(number) => NodeView::Number { number: *number },
                ValueNode::String(text) => NodeView::String { text },
                ValueNode::Symbol(text) => NodeView::Symbol { text },
                ValueNode::Tuple { arity } => NodeView::Tuple { arity: *arity },
                ValueNode::Function {
                    name,
                    sign: value_sign,
                    arity,
                } => NodeView::Function {
                    name,
                    sign: sign(*value_sign),
                    arity: *arity,
                },
                ValueNode::Supremum => NodeView::Supremum,
            };
            nodes.serialize_element(&view)?;
        }
        nodes.end()
    }
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum NodeView<'a> {
    Infimum,
    Number {
        number: i32,
    },
    String {
        text: &'a str,
    },
    Symbol {
        text: &'a str,
    },
    Tuple {
        arity: usize,
    },
    Function {
        name: &'a str,
        sign: &'static str,
        arity: usize,
    },
    Supremum,
}
const fn sign(value: Sign) -> &'static str {
    match value {
        Sign::Positive => "positive",
        Sign::Negative => "negative",
    }
}

pub(super) fn fingerprint(fixture: &Fixture) -> Result<String, Error> {
    let mut writer = SubjectHash {
        hash: Sha256::new(),
        bytes: 0,
    };
    serde_json::to_writer(&mut writer, &Subject(fixture))
        .map_err(|error| Error::Output(io::Error::other(error)))?;
    Ok(format!("{:x}", writer.hash.finalize()))
}
struct SubjectHash {
    hash: Sha256,
    bytes: usize,
}
impl Write for SubjectHash {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > MAX_SUBJECT_BYTES - self.bytes {
            return Err(io::Error::other(
                "relation subject exceeds 64 MiB serialization ceiling",
            ));
        }
        self.hash.update(buffer);
        self.bytes += buffer.len();
        Ok(buffer.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
