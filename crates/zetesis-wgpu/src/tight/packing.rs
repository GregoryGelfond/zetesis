//! Pure bounded layout, canonical certificate packing and ordered result decoding.

use super::update::{self, ROUTE_BEYOND_THRESHOLD, ROUTE_COMPLETE, ROUTE_DENSE, ROUTE_UPDATE};
use super::{TightGpuCheck, TightGpuLimits, TightGpuRoutes, TightSupport, poll};
use crate::formula_graph::dependency;
use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Theory, TightPlan, TightVerdict};

pub(super) const PARAM_BYTES: u64 = 64;
pub(super) const RESULT_WORDS: usize = 9;
pub(super) const RESULT_MAGIC: u32 = 0x5453_5031;
pub(super) const RESULT_GROUPED_MAGIC: u32 = 0x5453_4731;

const STATUS_STABLE: u32 = 0;
const STATUS_NOT_MODEL: u32 = 1;
const STATUS_RESIDUAL: u32 = 2;

fn capacity(detail: &str) -> GpuError {
    GpuError::new(GpuErrorKind::Capacity, detail)
}
fn address(value: usize) -> Result<u32, GpuError> {
    u32::try_from(value).map_err(|_| capacity("tight dimension exceeds u32"))
}
fn sum(values: &[u64]) -> Result<u64, GpuError> {
    values.iter().try_fold(0u64, |total, next| {
        total
            .checked_add(*next)
            .ok_or_else(|| capacity("tight byte sum overflow"))
    })
}
fn buffer(bytes: u64, device: &wgpu::Limits) -> Result<(), GpuError> {
    if bytes > device.max_buffer_size || bytes > device.max_storage_buffer_binding_size {
        return Err(capacity("tight storage exceeds granted device limits"));
    }
    Ok(())
}
fn vector<T>(length: usize) -> Result<Vec<T>, GpuError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| GpuError::new(GpuErrorKind::Allocation, "tight host reservation failed"))?;
    Ok(output)
}
fn words(bytes: u64) -> Result<Vec<u32>, GpuError> {
    vector(usize::try_from(bytes / 4).map_err(|_| capacity("tight buffer exceeds host"))?)
}

fn producer_storage(producers: u32, words: u32, support: TightSupport) -> Result<u64, GpuError> {
    let producer_words = u64::from(producers) * 4
        + if support == TightSupport::Grouped {
            u64::from(words) + 1
        } else {
            0
        };
    // The shader addresses records and appended offsets with u32 indices,
    // independently of a device's hypothetical byte capacity.
    if producer_words > u64::from(u32::MAX) {
        return Err(capacity("tight producer indexing exceeds u32"));
    }
    Ok(producer_words.max(4) * 4)
}

pub(super) struct Packed {
    pub(super) nodes: Vec<u32>,
    pub(super) roots: Vec<u32>,
    pub(super) producers: Vec<u32>,
}

#[derive(Clone, Copy)]
pub(super) struct Packing<'a> {
    pub(super) device: &'a wgpu::Limits,
    pub(super) support: TightSupport,
    pub(super) workgroup_sizing: crate::GpuWorkgroupSizing,
}

pub(super) struct Graph {
    pub(super) theory: Theory,
    pub(super) atoms: u32,
    pub(super) nodes: u32,
    pub(super) roots: u32,
    pub(super) edges: u32,
    pub(super) wide_words: u32,
    pub(super) levels: u32,
    pub(super) rows: crate::formula_graph::Rows,
    pub(super) producers: u32,
    pub(super) words: u32,
    pub(super) node_bytes: u64,
    pub(super) root_bytes: u64,
    pub(super) producer_bytes: u64,
    pub(super) bytes: u64,
    /// Simultaneous host packing, including temporary grouped-write cursors.
    pub(super) packing_bytes: u64,
    pub(super) work: u32,
    /// The complete scan's numeric-row units, a part of `work`.
    pub(super) numeric_work: u32,
    /// The owner column of the live numeric rows, retained with a schedule the
    /// update route engages until its readers index is built or refused.
    pub(super) owners: Vec<(u32, u32)>,
    /// Host bytes the owner column holds or, before extraction, will hold.
    pub(super) owner_bytes: u64,
    /// The most producers of one head, counted while packing a schedule the
    /// update route engages; it decides the update state's count width without
    /// a later allocation.
    pub(super) largest_producers: u32,
    /// Dense evaluation's strided iterations over the level widths at the
    /// dispatched workgroup size, `sum(ceil(width / W))`; zero when serial.
    pub(super) strided_depth: u64,
    /// The workgroup size this graph dispatches: the fixed size, or the size
    /// its packed schedule fits.
    pub(super) workgroup_size: crate::GpuWorkgroupSize,
    support: TightSupport,
    workgroup_sizing: crate::GpuWorkgroupSizing,
}
impl Graph {
    const fn result_marker(&self) -> u32 {
        match self.support {
            TightSupport::Atomic => RESULT_MAGIC,
            TightSupport::Grouped => RESULT_GROUPED_MAGIC,
        }
    }

    #[cfg(test)]
    pub(super) fn new(certificate: &TightPlan, packing: Packing<'_>) -> Result<Self, GpuError> {
        Self::with_control(certificate, packing, u64::MAX, &Cancellation::default())
    }

    pub(super) fn with_control(
        certificate: &TightPlan,
        packing: Packing<'_>,
        max_staging_bytes: u64,
        cancellation: &Cancellation,
    ) -> Result<Self, GpuError> {
        let theory = certificate.theory();
        let atoms = address(theory.atom_count())?;
        let graph = crate::formula_graph::Shape::with_control(
            theory,
            packing.device,
            max_staging_bytes,
            cancellation,
        )?;
        let nodes = graph.nodes;
        let roots = address(theory.roots().len())?;
        let producers = address(certificate.producers().len())?;
        // Reserve the maximum kernel stride after each loop's last valid address.
        if [atoms, nodes, roots, producers]
            .into_iter()
            .any(|n| !crate::kernel::fits_strided_loop(n))
        {
            return Err(capacity("tight strided loop increment exceeds u32"));
        }
        let words = atoms.div_ceil(32);
        let node_bytes = graph.bytes;
        let root_bytes = u64::from(roots.max(1)) * 4;
        let grouped = packing.support == TightSupport::Grouped;
        let producer_bytes = producer_storage(producers, words, packing.support)?;
        for bytes in [node_bytes, root_bytes, producer_bytes] {
            buffer(bytes, packing.device)?;
        }
        let bytes = sum(&[node_bytes, root_bytes, producer_bytes])?;
        let packing_bytes = sum(&[
            bytes,
            graph.rows.bytes()?,
            if grouped { u64::from(words) * 4 } else { 0 },
        ])?;
        let mut shape = Self {
            theory: theory.clone(),
            atoms,
            nodes,
            roots,
            edges: graph.edges,
            wide_words: graph.wide_words,
            levels: 0,
            rows: graph.rows,
            producers,
            words,
            node_bytes,
            root_bytes,
            producer_bytes,
            bytes,
            packing_bytes,
            work: 0,
            numeric_work: graph.numeric_work,
            owners: Vec::new(),
            owner_bytes: 0,
            largest_producers: 0,
            strided_depth: 0,
            workgroup_size: crate::kernel::base_size(packing.workgroup_sizing),
            support: packing.support,
            workgroup_sizing: packing.workgroup_sizing,
        };
        shape.work = words
            .checked_mul(2)
            .and_then(|n| n.checked_add(nodes))
            .and_then(|n| n.checked_add(shape.edges))
            .and_then(|n| n.checked_add(roots))
            .and_then(|n| n.checked_add(producers))
            .and_then(|n| n.checked_add(graph.numeric_work))
            .ok_or_else(|| capacity("tight complete-scan work exceeds u32"))?;
        Ok(shape)
    }

    fn pack_nodes(
        &self,
        certificate: &TightPlan,
        cancellation: &Cancellation,
    ) -> Result<Vec<u32>, GpuError> {
        if !self.theory.same_instance(certificate.theory()) {
            return Err(GpuError::new(
                GpuErrorKind::Seed,
                "certificate belongs to another Theory",
            ));
        }
        let mut nodes = words(self.node_bytes)?;
        crate::formula_graph::pack(&self.theory, &self.rows, &mut nodes, cancellation)?;
        Ok(nodes)
    }

    /// Complete an already-admitted serial shape. Optional level storage is
    /// selected only when its complete cold layout and work also fit.
    pub(super) fn prepare(
        &mut self,
        certificate: &TightPlan,
        limits: TightGpuLimits,
        device: &wgpu::Limits,
        plan: &mut Plan,
        cancellation: &Cancellation,
    ) -> Result<Packed, GpuError> {
        let mut nodes = self.pack_nodes(certificate, cancellation)?;
        let depths =
            dependency::label_depths(self.nodes, self.wide_words, &mut nodes, cancellation)?;
        let levels = dependency::parallel_levels(self.nodes, depths);
        if levels != 0 {
            let serial = (self.root_bytes, self.bytes, self.packing_bytes);
            let scheduled = (|| {
                let root_bytes =
                    u64::from(dependency::root_words(self.nodes, self.roots, levels)?) * 4;
                buffer(root_bytes, device)?;
                let added = root_bytes - self.root_bytes;
                self.bytes = sum(&[self.bytes, added])?;
                self.packing_bytes = sum(&[self.packing_bytes, added])?;
                self.root_bytes = root_bytes;
                self.levels = levels;
                // Only a scheduled theory can update, so only it reserves the
                // owner column of its live numeric rows and one transient word
                // per atom for counting producers per head. Whether the route
                // engages depends on the packed level widths; a narrow schedule
                // releases both once packed.
                self.owner_bytes = owner_bytes(self.rows.live_rows())?;
                self.packing_bytes = sum(&[self.packing_bytes, u64::from(self.atoms) * 4])?;
                Plan::new(self, plan.worlds as usize, limits, device, true, plan.epoch)
            })();
            match scheduled {
                Ok(selected) if selected.levels != 0 => *plan = selected,
                Ok(_) => {
                    (self.root_bytes, self.bytes, self.packing_bytes) = serial;
                    self.levels = 0;
                    self.owner_bytes = 0;
                }
                Err(error) if error.kind() == GpuErrorKind::Capacity => {
                    (self.root_bytes, self.bytes, self.packing_bytes) = serial;
                    self.levels = 0;
                    self.owner_bytes = 0;
                }
                Err(error) => return Err(error),
            }
        }
        let packed = self.pack_columns(certificate, nodes, cancellation)?;
        let widths = dependency::level_widths(&packed.roots, self.roots, self.levels)?;
        self.workgroup_size =
            crate::kernel::dispatch_size(self.workgroup_sizing, widths.clone(), device);
        let size = self.workgroup_size.invocations();
        self.strided_depth = widths.map(|width| u64::from(width.div_ceil(size))).sum();
        // Only a schedule the update route engages needs its inputs. A narrow
        // one releases what its admission reserved, and its plan, admitted
        // with more bytes, accounts exactly what remains.
        if update::engages(self.strided_depth, self.levels) {
            self.owners = self.rows.owners(cancellation)?;
            self.largest_producers = largest_producers(certificate, self.atoms, cancellation)?;
        } else if self.levels != 0 {
            self.owner_bytes = 0;
            self.packing_bytes -= u64::from(self.atoms) * 4;
            *plan = Plan::new(self, plan.worlds as usize, limits, device, true, plan.epoch)?;
        }
        Ok(packed)
    }

    #[cfg(test)]
    pub(super) fn pack(
        &self,
        certificate: &TightPlan,
        cancellation: &Cancellation,
    ) -> Result<Packed, GpuError> {
        self.pack_columns(
            certificate,
            self.pack_nodes(certificate, cancellation)?,
            cancellation,
        )
    }

    fn pack_columns(
        &self,
        certificate: &TightPlan,
        mut nodes: Vec<u32>,
        cancellation: &Cancellation,
    ) -> Result<Packed, GpuError> {
        let mut roots = words(self.root_bytes)?;
        dependency::pack(
            &self.theory,
            self.nodes,
            self.levels,
            &nodes,
            &mut roots,
            cancellation,
        )?;
        // Depths are preparation scratch. A scheduled theory's header word 3
        // holds each node's position in the level order instead, which the
        // update route's queued set is indexed by; a serial one holds zero.
        for header in nodes.chunks_exact_mut(4).take(self.nodes as usize) {
            poll(cancellation)?;
            header[3] = 0;
        }
        if self.levels != 0 {
            let order = (self.roots + self.levels + 1) as usize;
            for (position, &node) in roots[order..order + self.nodes as usize].iter().enumerate() {
                poll(cancellation)?;
                nodes[node as usize * 4 + 3] = address(position)?;
            }
        }
        let producers = self.pack_producers(certificate, cancellation)?;
        Ok(Packed {
            nodes,
            roots,
            producers,
        })
    }

    pub(super) fn pack_producers(
        &self,
        certificate: &TightPlan,
        cancellation: &Cancellation,
    ) -> Result<Vec<u32>, GpuError> {
        let mut output = words(self.producer_bytes)?;
        if self.support == TightSupport::Atomic {
            for producer in certificate.producers() {
                poll(cancellation)?;
                output.extend(producer_record(producer)?);
            }
            if output.is_empty() {
                output.extend([0; 4]);
            }
            return Ok(output);
        }
        let length = usize::try_from(self.producer_bytes / 4)
            .map_err(|_| capacity("tight producers exceed host"))?;
        output.resize(length, 0);
        let offsets = self.producers as usize * 4;
        // Counts occupy the next group's slot. Prefix summation produces W+1
        // half-open offsets, including zero-length groups and the final bound P.
        for producer in certificate.producers() {
            poll(cancellation)?;
            output[offsets + producer.head() / 32 + 1] += 1;
        }
        for word in 0..self.words as usize {
            poll(cancellation)?;
            let prefix = output[offsets + word];
            output[offsets + word + 1] += prefix;
        }
        let mut cursors = vector(self.words as usize)?;
        cursors.extend_from_slice(&output[offsets..offsets + self.words as usize]);
        // Each canonical occurrence advances precisely its word's cursor once.
        // Thus all occurrences are retained, in original order within a group.
        for producer in certificate.producers() {
            poll(cancellation)?;
            let cursor = &mut cursors[producer.head() / 32];
            let start = *cursor as usize * 4;
            output[start..start + 4].copy_from_slice(&producer_record(producer)?);
            *cursor += 1;
        }
        Ok(output)
    }
}

/// The most producers of one head, with one transient counter per atom.
fn largest_producers(
    certificate: &TightPlan,
    atoms: u32,
    cancellation: &Cancellation,
) -> Result<u32, GpuError> {
    let mut heads = vector::<u32>(atoms as usize)?;
    heads.resize(atoms as usize, 0);
    let mut largest = 0;
    for producer in certificate.producers() {
        poll(cancellation)?;
        let count = &mut heads[producer.head()];
        *count = count
            .checked_add(1)
            .ok_or_else(|| capacity("tight producer count overflows"))?;
        largest = largest.max(*count);
    }
    Ok(largest)
}

fn owner_bytes(rows: usize) -> Result<u64, GpuError> {
    rows.checked_mul(size_of::<(u32, u32)>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(|| capacity("tight owner column exceeds host"))
}

fn producer_record(producer: &zetesis_ferraris::TightProducer) -> Result<[u32; 4], GpuError> {
    let (body, present) = match producer.body() {
        Some(body) => (address(body)?, 1),
        None => (0, 0),
    };
    Ok([address(producer.head())?, body, present, 0])
}

/// The per-slot update state a plan lays out, and the index bytes it admits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StateLayout {
    /// Words of each slot's state.
    pub(super) slot_words: u32,
    /// Resident index bytes not yet part of the graph: the bound of an index
    /// this batch builds, zero once it is resident.
    pub(super) index_bytes: u64,
    /// Transient host bytes of building the index in this batch.
    pub(super) build_bytes: u64,
}

/// The update route's per-batch parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Route {
    /// The update route may run, and the complete and dense routes seed the
    /// state for it. Requires bound state and a parallel schedule.
    pub(super) allowed: bool,
    pub(super) threshold: u32,
    pub(super) readers: u32,
    pub(super) atom_nodes: u32,
    pub(super) count_bits: u32,
    pub(super) slot_words: u32,
    /// The fewest operations an update performs.
    pub(super) floor: u32,
}

pub(super) struct Plan {
    pub(super) worlds: u32,
    pub(super) seeds: u64,
    pub(super) truth: u64,
    pub(super) support: u64,
    pub(super) results: u64,
    /// The update state's bytes, or zero when the placeholder is bound.
    pub(super) state: u64,
    pub(super) transport: u64,
    pub(super) accounted: u64,
    /// The complete or dense route's fixed charge per candidate, including the
    /// state's writes when it is bound.
    pub(super) work: u32,
    /// Work reserved per candidate: `work`, or the update route's worst case
    /// when that is larger and slots may update.
    pub(super) allowance: u32,
    pub(super) levels: u32,
    pub(super) epoch: u32,
    pub(super) original_truth_reused: bool,
    pub(super) route: Route,
}
impl Plan {
    pub(super) fn new(
        graph: &Graph,
        worlds: usize,
        limits: TightGpuLimits,
        device: &wgpu::Limits,
        fresh: bool,
        epoch: u32,
    ) -> Result<Self, GpuError> {
        Self::with_state(graph, worlds, limits, device, fresh, epoch, None)
    }

    /// Lay out a batch, with each slot's update state when `state` is given.
    /// The state is part of the per-slot footprint the byte ceiling sees.
    pub(super) fn with_state(
        graph: &Graph,
        worlds: usize,
        limits: TightGpuLimits,
        device: &wgpu::Limits,
        fresh: bool,
        epoch: u32,
        state: Option<StateLayout>,
    ) -> Result<Self, GpuError> {
        let count = address(worlds)?;
        crate::runtime::positive_timeout(limits.timeout)?;
        if worlds > limits.max_candidates || count > device.max_compute_workgroups_per_dimension {
            return Err(capacity("tight batch exceeds candidate/dispatch ceiling"));
        }
        if limits.max_work_per_candidate < u64::from(graph.work) || epoch == 0 {
            return Err(capacity("tight full-scan work or epoch exceeds limits"));
        }
        // A resident schedule may be bypassed under a tighter work allowance.
        // Its real retained storage is still charged below.
        let (levels, work) = graph
            .work
            .checked_add(graph.levels)
            .filter(|work| u64::from(*work) <= limits.max_work_per_candidate)
            .map_or((0, graph.work), |work| (graph.levels, work));
        let array = |length: u32| -> Result<u64, GpuError> {
            let elements = u64::from(length) * u64::from(count);
            if elements > u64::from(u32::MAX) {
                return Err(capacity("tight world offset exceeds u32"));
            }
            Ok(elements.max(1) * 4)
        };
        let seeds = array(graph.words)?;
        let truth = array(graph.nodes)?;
        let support = array(graph.words)?;
        let results = array(address(RESULT_WORDS)?)?;
        let state_bytes = match state {
            Some(layout) => array(layout.slot_words)?,
            None => 0,
        };
        for bytes in [seeds, truth, support, results, state_bytes] {
            buffer(bytes, device)?;
        }
        if device.max_uniform_buffer_binding_size < PARAM_BYTES
            || device.max_buffer_size < PARAM_BYTES
        {
            return Err(capacity(
                "tight uniform buffer exceeds granted device limits",
            ));
        }
        let bound = if state.is_some() {
            state_bytes
        } else {
            update::PLACEHOLDER_BYTES
        };
        let transport = sum(&[PARAM_BYTES, seeds, truth, support, results, results, bound])?;
        let host_results = u64::from(count) * std::mem::size_of::<TightGpuCheck>() as u64;
        let (index_bytes, build_bytes) =
            state.map_or((0, 0), |layout| (layout.index_bytes, layout.build_bytes));
        let accounted = sum(&[
            graph.bytes,
            index_bytes,
            graph.owner_bytes,
            transport,
            PARAM_BYTES,
            seeds,
            host_results,
            if fresh { graph.packing_bytes } else { 0 },
            build_bytes,
        ])?;
        if worlds != 0 && accounted > limits.max_batch_bytes {
            return Err(capacity("tight batch exceeds authored byte ceiling"));
        }
        Ok(Self {
            worlds: count,
            seeds,
            truth,
            support,
            results,
            state: state_bytes,
            transport,
            accounted,
            work,
            allowance: work,
            levels,
            epoch,
            original_truth_reused: false,
            route: Route::default(),
        })
    }

    /// Keep the already-admitted complete scan when optional change detection
    /// cannot fit. The bound includes a header decision, old-value comparisons,
    /// changed-bit publication, and every operand-change inspection; dense
    /// changes can still require the entire original reduction afterwards.
    pub(super) fn reuse_original_truth(&mut self, graph: &Graph, ceiling: u64) {
        let work = graph
            .nodes
            .checked_mul(2)
            .and_then(|n| n.checked_add(graph.edges))
            .and_then(|n| n.checked_add(4))
            .and_then(|extra| self.work.checked_add(extra));
        if let Some(work) = work.filter(|work| u64::from(*work) <= ceiling) {
            self.work = work;
            self.allowance = work;
            self.original_truth_reused = true;
        }
    }

    /// Decide whether this plan, which binds the update state of `index`,
    /// allows the update route, and charge the routes accordingly.
    ///
    /// The route is allowed when the schedule is parallel and its reservation
    /// fits `ceiling`: the larger of the update's worst case and dense reuse
    /// after a change count, with the state's writes. Allowed, the complete and
    /// dense routes write the slot's retained candidate and clear its summary,
    /// `words + 1` operations; dense reuse after a change count adds the count's
    /// `2 * words`, and with reuse authority every slot may take either route
    /// or update, so each reserves the largest. Not allowed, no route writes
    /// the state and every charge is the stateless one.
    pub(super) fn bind_state(
        &mut self,
        graph: &Graph,
        index: &update::Index,
        ceiling: u64,
    ) -> bool {
        let words = u64::from(graph.words);
        let worst = update::worst_case(
            graph,
            self.levels,
            index.count_bits,
            index.entries,
            index.atom_entries,
        );
        let dense = u64::from(graph.work)
            + u64::from(self.levels)
            + 2 * u64::from(graph.nodes)
            + u64::from(graph.edges)
            + 4
            + 3 * words
            + 1;
        let allowed = self.levels != 0
            && worst.is_some_and(|worst| worst.max(dense) <= ceiling.min(u64::from(u32::MAX)));
        self.route = Route {
            allowed,
            threshold: update::threshold(graph.atoms),
            readers: index.readers,
            atom_nodes: index.atom_nodes,
            count_bits: index.count_bits,
            slot_words: index.slot_words,
            floor: u32::try_from(update::floor(graph, self.levels)).unwrap_or(u32::MAX),
        };
        if allowed {
            // The gate bounds every charge below by `ceiling` within u32.
            self.work += graph.words + 1;
            self.allowance = if self.original_truth_reused {
                let counted = self.work + 2 * graph.words;
                counted.max(
                    worst
                        .and_then(|worst| u32::try_from(worst).ok())
                        .unwrap_or(counted),
                )
            } else {
                self.work
            };
        }
        allowed
    }

    pub(super) fn params(&self, graph: &Graph) -> [u32; 16] {
        [
            graph.atoms,
            graph.nodes,
            graph.roots,
            graph.producers,
            graph.words,
            self.levels,
            self.work,
            self.epoch,
            self.route.threshold,
            u32::from(self.route.allowed),
            self.route.readers,
            self.route.atom_nodes,
            self.route.count_bits,
            self.route.slot_words,
            0,
            0,
        ]
    }

    pub(super) fn pack(
        &self,
        graph: &Graph,
        candidates: &[Interpretation],
        staging: &mut crate::candidates::Staging,
        ceiling: u64,
        cancellation: &zetesis_cpu::Cancellation,
    ) -> Result<u64, GpuError> {
        if candidates.len() != self.worlds as usize {
            staging.release();
            return Err(capacity("tight candidate count differs from admitted plan"));
        }
        staging.pack(
            &graph.theory,
            candidates,
            self.seeds,
            self.accounted,
            ceiling,
            cancellation,
        )
    }
}

/// A batch's decoded checks with its route receipts.
#[derive(Debug)]
pub(super) struct Decoded {
    pub(super) checks: Vec<TightGpuCheck>,
    pub(super) routes: TightGpuRoutes,
    pub(super) nodes_evaluated: u64,
    /// The sum of the work each check reports performed.
    pub(super) work: u64,
}

/// Validate every record against the batch and decode it in order.
///
/// A slot without reuse authority takes the complete route. With authority it
/// takes dense reuse unless the update route is allowed; then a slot seeded by
/// its previous record counts its change and updates, or takes dense reuse
/// beyond the threshold. Complete and dense records charge the plan's fixed
/// work, dense reuse beyond the threshold adds the count's `2 * words`, and none
/// evaluates a node by update; an update charges at least its floor and at most
/// the reservation, and evaluates at most every node. Every record is seeded
/// exactly when the route was allowed. Any other record is an invalid readback.
pub(super) fn decode(
    words: &[u32],
    graph: &Graph,
    plan: &Plan,
    seeds: &[u32],
    cancellation: &Cancellation,
) -> Result<Decoded, GpuError> {
    let fail = || GpuError::new(GpuErrorKind::Readback, "invalid tight result record");
    if words.len() != plan.worlds as usize * RESULT_WORDS
        || u64::try_from(seeds.len())
            .ok()
            .and_then(|n| n.checked_mul(4))
            != Some(plan.seeds)
    {
        return Err(fail());
    }
    let mut checks = vector(plan.worlds as usize)?;
    let mut routes = TightGpuRoutes::default();
    let mut nodes_evaluated = 0_u64;
    let mut work = 0_u64;
    for (world, row) in words.chunks_exact(RESULT_WORDS).enumerate() {
        poll(cancellation)?;
        if row[0] != plan.epoch || row[1] as usize != world || row[5] != graph.result_marker() {
            return Err(fail());
        }
        let (performed, route, evaluated, seeded) = (row[4], row[6], row[7], row[8]);
        let reused = plan.original_truth_reused;
        let updating = reused && plan.route.allowed;
        let receipt = match route {
            ROUTE_COMPLETE => !reused && performed == plan.work && evaluated == 0,
            ROUTE_DENSE => reused && performed == plan.work && evaluated == 0,
            ROUTE_BEYOND_THRESHOLD => {
                updating
                    && plan.work.checked_add(2 * graph.words) == Some(performed)
                    && evaluated == 0
            }
            ROUTE_UPDATE => {
                updating
                    && evaluated <= graph.nodes
                    && plan.route.floor <= performed
                    && performed <= plan.allowance
            }
            _ => false,
        };
        if !receipt || seeded != u32::from(plan.route.allowed) {
            return Err(fail());
        }
        let verdict = match (row[2], row[3]) {
            (STATUS_STABLE, 0) => TightVerdict::Stable,
            (STATUS_NOT_MODEL, ordinal) if ordinal < graph.roots => TightVerdict::NotModel {
                root: graph.theory.roots()[ordinal as usize],
            },
            (STATUS_RESIDUAL, atom)
                if atom < graph.atoms
                    && seeds[world * graph.words as usize + atom as usize / 32]
                        & (1 << (atom % 32))
                        != 0 =>
            {
                TightVerdict::Residual {
                    unsupported_atom: atom as usize,
                }
            }
            _ => return Err(fail()),
        };
        match route {
            ROUTE_COMPLETE => routes.complete += 1,
            ROUTE_DENSE => routes.dense_reuse += 1,
            ROUTE_BEYOND_THRESHOLD => {
                routes.dense_reuse += 1;
                routes.beyond_threshold += 1;
            }
            _ => routes.update += 1,
        }
        nodes_evaluated += u64::from(evaluated);
        work += u64::from(performed);
        checks.push(TightGpuCheck {
            verdict,
            work: u64::from(performed),
        });
    }
    Ok(Decoded {
        checks,
        routes,
        nodes_evaluated,
        work,
    })
}

#[cfg(test)]
mod tests;
