use super::{Case, Configuration, ConstructionLimits, Error};
use serde::{Serialize, Serializer, ser::SerializeSeq};
use zetesis_ferraris::Node;

/// Distinct measured algorithms; guarded SAT uses witnesses acquired beforehand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// Every bit-ordered interpretation receives an original native check.
    Direct,
    /// The same interpretations first test retained conditional guards.
    Feedback,
    /// Ordinary native stable-model iterator, without additional guards.
    Search,
    /// Ordinary iterator with pre-acquired guards installed after its first answer.
    Restricted,
}

/// Nonoverlapping stage intervals nested within a whole-route wall interval.
/// Fixture/reference preparation, parity validation and publication are excluded.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct StageTimes {
    /// Native standalone checking or iterator next calls.
    pub membership_ns: u128,
    /// Guard deduplication and complete construction, including refused attempts.
    pub construction_ns: u128,
    /// Conditional guard evaluation, including explicit same-owner rebinding.
    pub application_ns: u128,
    /// Original iterator construction and its initial encoding.
    pub search_setup_ns: u128,
    /// Existing restriction encoding and cursor restart calls.
    pub installation_ns: u128,
    /// Entire route, including orchestration and its own record/vector setup.
    pub total_ns: u128,
}

/// Completed and attempted work from the experiment's single guard owner.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Construction {
    /// Cumulative admitted construction/deduplication/publication steps.
    pub work: u64,
    /// Successfully retained distinct witness guards.
    pub guards: usize,
    /// Nodes belonging to successfully retained guard DAGs.
    pub nodes: usize,
    /// Retained entry/node/root/witness vector capacity; no native/Arc/RSS claim.
    pub retained_bytes: usize,
    /// Greatest observed actual single-build vector capacity. Refused proposals
    /// are excluded; allocated capacity is retained in the receipt on readback refusal.
    pub peak_build_bytes: usize,
    /// Greatest observed retained-plus-build capacity; never add it to retained.
    pub peak_live_bytes: usize,
}

/// Existing cumulative SAT receipts, present only for iterator routes. Work is
/// the native schedule, not inferred from standalone membership call counts.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Native {
    /// Cumulative SAT search/encoding work, including restriction work.
    pub work: u64,
    /// Classical candidate queries, including final UNSAT.
    pub candidate_queries: u64,
    /// Classical candidates admitted by the native iterator.
    pub candidates: u64,
    /// Original frozen-reduct membership queries.
    pub countermodel_queries: u64,
    /// Independently validated proper-subset witnesses found.
    pub countermodels: u64,
    /// Successfully appended restrictions; each restarts the candidate cursor.
    pub restrictions: u64,
    /// Distinct exact exclusion keys retained across restarts.
    pub projection_entries: u64,
    /// Peak named native projection capacity, separate from guard vectors.
    pub projection_peak_bytes: u128,
}
impl From<zetesis_sat::Statistics> for Native {
    fn from(value: zetesis_sat::Statistics) -> Self {
        Self {
            work: value.search.work,
            candidate_queries: value.candidate_queries,
            candidates: value.candidates,
            countermodel_queries: value.countermodel_queries,
            countermodels: value.countermodels,
            restrictions: value.candidate_restrictions,
            projection_entries: value.projections.entries,
            projection_peak_bytes: value.projections.peak_bytes,
        }
    }
}

/// Actual route prefix. Exact original atom IDs are encoded as lossless bitsets
/// over the serialized universe (at most six atoms); no projected display family.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Progress {
    /// Bit-ordered candidates entered; zero for native iterator routes.
    pub candidates_started: u64,
    /// Candidates with completed membership or an established guard rejection.
    pub candidates_completed: u64,
    /// Standalone native membership calls entered, including an incomplete attempt.
    pub membership_calls: u64,
    /// Standalone native membership calls that established a verdict.
    pub membership_completed: u64,
    /// Original classical-model failures among completed standalone calls.
    pub not_models: u64,
    /// Completed standalone calls with an actual proper-subset witness.
    pub nonminimal: u64,
    /// Candidates rejected by a retained conditional witness; saved native calls.
    pub filtered: u64,
    /// Conditional guard evaluations entered, including any incomplete attempt.
    pub guard_evaluations: u64,
    /// Repeated checked witnesses needing no second guard.
    pub duplicate_witnesses: u64,
    /// Existing SAT restriction calls entered, including any refused attempt.
    pub installation_attempts: u64,
    /// Successfully installed restrictions and actual candidate-cursor restarts.
    pub restarts: u64,
    /// Exact completed stable interpretations, in their delivered order.
    /// On failure this is a checked prefix, not an exhaustive family.
    pub stable: Vec<u64>,
    /// Same-owner witness IDs learned here or borrowed by the restricted route.
    /// This output view is separate from the guard storage receipt.
    pub witnesses: Vec<u64>,
    /// Guards borrowed from the immediately preceding feedback replay.
    pub pre_acquired_guards: usize,
    /// Borrowed pre-acquired guard nodes; their construction is not retimed.
    pub pre_acquired_nodes: usize,
    /// Actual entry/node/root/witness vector capacity retained by that guard owner.
    /// Separate from current construction and native projection receipts.
    pub pre_acquired_bytes: usize,
    /// Construction receipts, absent work is zero rather than native SAT work.
    pub construction: Construction,
    /// Native iterator receipts, absent on direct/feedback fixed-candidate routes.
    pub native: Option<Native>,
    /// Complete candidate coverage only after this route's normal completion.
    pub exhausted: bool,
}

/// One complete route observation, qualified against the entire reference family.
#[derive(Clone, Debug, Serialize)]
pub struct Sample {
    /// Fixed original owner.
    pub case: Case,
    /// Actual measured algorithm.
    pub route: Route,
    /// Qualification, warmup or timed population.
    pub phase: &'static str,
    /// Zero-based repetition within its population.
    pub repetition: usize,
    /// Original work, checked family and completion receipts.
    pub progress: Progress,
    /// Nested wall intervals; not worker intervals or process time/RSS.
    pub elapsed: StageTimes,
}

/// Actual fixed native limits derived from the configuration's native request.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct NativeLimits {
    /// CNF variables including auxiliary gates.
    pub variables: usize,
    /// CNF clauses including installed restrictions.
    pub clauses: usize,
    /// CNF literal occurrences.
    pub literals: usize,
    /// Distinct projection-history entries.
    pub projection_entries: u64,
    /// Logical projection-history nodes.
    pub projection_nodes: usize,
    /// Named projection-history byte envelope.
    pub projection_bytes: usize,
    /// Cumulative search and encoding work per native operation.
    pub work: u64,
    /// Native decisions per operation.
    pub decisions: u64,
    /// Classical candidates per iterator.
    pub candidates: u64,
    /// Independent verification work per native evaluation call.
    pub verification_work: u64,
}
impl From<zetesis_sat::Limits> for NativeLimits {
    fn from(value: zetesis_sat::Limits) -> Self {
        Self {
            variables: value.admission.max_variables,
            clauses: value.admission.max_clauses,
            literals: value.admission.max_literals,
            projection_entries: value.projections.max_entries,
            projection_nodes: value.projections.max_nodes,
            projection_bytes: value.projections.max_bytes,
            work: value.search.max_work,
            decisions: value.search.max_decisions,
            candidates: value.max_candidates,
            verification_work: value.max_verification_work,
        }
    }
}

/// Synchronous experiment event. A retaining consumer must clone a sample;
/// borrowing keeps publication outside the measured intervals.
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event<'a> {
    /// Finite configuration; fixed native limits are part of this schema contract.
    Start {
        /// Version of the finite event schema.
        schema: u32,
        /// Actual requested bounds and populations.
        configuration: &'a Configuration,
        /// Native request actually passed to checking and search.
        native_limits: NativeLimits,
        /// Separate fixed guard maxima used by all-J/all-X qualification.
        qualification_construction: ConstructionLimits,
        /// Fixed exhaustive proper-subset limit per reference call.
        reference_subsets: u64,
    },
    /// Exact authored DAG, reference family and all-J/all-X refinement census.
    Qualified {
        /// Fixed owner label.
        case: Case,
        /// Complete original universe.
        atoms: usize,
        /// Topological source nodes as [kind,left,right], kinds atom/false/and/or/implies.
        #[serde(serialize_with = "self::nodes")]
        nodes: &'a [Node],
        /// Original asserted roots, preserving duplicates.
        roots: &'a [usize],
        /// Every stable reference interpretation as an exact universe bitset.
        stable: &'a [u64],
        /// All ordered witness/candidate pairs, including non-subsets.
        pairs: u64,
    },
    /// Complete route observation; no partial sample is emitted as passing.
    Sample(&'a Sample),
    /// Original typed error is returned by measure; this view renders its message.
    Failed {
        /// Incomplete current route, retaining its exact completed prefix.
        sample: &'a Sample,
        /// Human-readable rendering of the original error.
        #[serde(serialize_with = "self::error")]
        error: &'a Error,
    },
    /// All qualification and requested route observations completed.
    Complete {
        /// Number of qualified complete route observations.
        samples: usize,
    },
}
fn nodes<S: Serializer>(nodes: &[Node], serializer: S) -> Result<S::Ok, S::Error> {
    let mut sequence = serializer.serialize_seq(Some(nodes.len()))?;
    for node in nodes {
        let record = match *node {
            Node::Atom(atom) => ("atom", atom, 0),
            Node::False => ("false", 0, 0),
            Node::And(a, b) => ("and", a, b),
            Node::Or(a, b) => ("or", a, b),
            Node::Implies(a, b) => ("implies", a, b),
        };
        sequence.serialize_element(&record)?;
    }
    sequence.end()
}
fn error<S: Serializer>(error: &Error, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(error)
}
