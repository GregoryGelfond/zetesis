//! Checked host packing for the WGSL ABI. Immutable graph dimensions are
//! validated once per resident program; batch transport is bounded separately.

use std::num::NonZeroU32;

use zetesis_core::{GroundProgram, Program, Seed};

use crate::{GpuCheck, GpuError, GpuErrorKind, GpuLimits, MAX_ATOMS, UNIFORM_BYTES};

const RULE_WORDS: usize = 8;
const WORD_BYTES: u64 = 4;
const RECEIPT_WORDS: usize = 4;
const COMPLETION_MARKER: u32 = 0x5352_4331;

pub(crate) fn next_epoch(previous: u32) -> Result<NonZeroU32, GpuError> {
    previous
        .checked_add(1)
        .and_then(NonZeroU32::new)
        .ok_or_else(|| GpuError::new(GpuErrorKind::Capacity, "static submission epoch exhausted"))
}

pub(crate) struct GraphPlan {
    program: Program,
    atom_count: u32,
    word_count: usize,
    rule_count: u32,
    rule_words: usize,
    antecedent_words: usize,
    carrier_words: usize,
    pub(crate) resident_bytes: u64,
}

impl GraphPlan {
    pub(crate) fn new(program: &GroundProgram, device: &wgpu::Limits) -> Result<Self, GpuError> {
        capacity("atoms", program.atom_count(), MAX_ATOMS)?;
        let atom_count = address("atom count", program.atom_count())?;
        let rule_count = address("rule count", program.rules().len())?;
        // Each strided rule scan adds 64 after a valid index. Rule storage
        // usually imposes a far smaller limit, but the address proof is local.
        if rule_count > u32::MAX - crate::WORKGROUP_SIZE {
            return Err(GpuError::new(
                GpuErrorKind::Capacity,
                "rule index increment would overflow u32",
            ));
        }
        let word_count = program.word_count();
        if word_count != program.atom_count().div_ceil(32) {
            return Err(GpuError::new(
                GpuErrorKind::Validation,
                "compiled program has an inconsistent closure-word count",
            ));
        }
        let rule_words =
            multiply("rule metadata", program.rules().len(), RULE_WORDS)?.max(RULE_WORDS);
        let mut antecedent_words = 0usize;
        for rule in program.rules() {
            for list in [rule.positive(), rule.gate_true(), rule.gate_false()] {
                antecedent_words = add("antecedents", antecedent_words, list.len())?;
            }
        }
        address("antecedent array", antecedent_words)?;
        let antecedent_words = antecedent_words.max(1);
        let carrier_words = word_count.max(1);
        let mut resident_bytes = 0u64;
        for (name, words) in [
            ("rule metadata", rule_words),
            ("antecedents", antecedent_words),
            ("gate carrier", carrier_words),
        ] {
            let size = bytes(words)?;
            buffer_capacity(name, size, device)?;
            resident_bytes = add_bytes(resident_bytes, size)?;
        }
        Ok(Self {
            program: program.program().clone(),
            atom_count,
            word_count,
            rule_count,
            rule_words,
            antecedent_words,
            carrier_words,
            resident_bytes,
        })
    }

    pub(crate) fn matches(&self, program: &GroundProgram) -> bool {
        // GroundProgram::compile is deterministic for an immutable Program;
        // its limits only reject construction, never change the resulting graph.
        // Retaining the Arc-backed Program prevents pointer-identity reuse.
        self.program.same_instance(program.program())
    }
}

pub(crate) struct BatchPlan {
    pub(crate) epoch: NonZeroU32,
    pub(crate) atom_count: u32,
    pub(crate) word_count: usize,
    pub(crate) rule_count: u32,
    pub(crate) world_count: u32,
    seed_words: usize,
    pub(crate) seed_bytes: u64,
    pub(crate) result_bytes: u64,
    result_words: usize,
    pub(crate) transport_bytes: u64,
    pub(crate) accounted_bytes: u64,
}

impl BatchPlan {
    #[cfg(test)]
    fn new(
        program: &GroundProgram,
        worlds: usize,
        budget: GpuLimits,
        device: &wgpu::Limits,
    ) -> Result<Self, GpuError> {
        Self::for_graph(
            &GraphPlan::new(program, device)?,
            worlds,
            budget,
            device,
            true,
            NonZeroU32::MIN,
        )
    }

    pub(crate) fn for_graph(
        graph: &GraphPlan,
        worlds: usize,
        budget: GpuLimits,
        device: &wgpu::Limits,
        upload_graph: bool,
        epoch: NonZeroU32,
    ) -> Result<Self, GpuError> {
        capacity("candidates", worlds, budget.max_candidates)?;
        let candidate_count = address("candidate count", worlds)?;
        if candidate_count > device.max_compute_workgroups_per_dimension {
            return Err(GpuError::new(
                GpuErrorKind::Capacity,
                "candidate count exceeds the device's dispatch dimension",
            ));
        }
        if budget.timeout.is_zero() {
            return Err(GpuError::new(
                GpuErrorKind::Capacity,
                "the GPU wait timeout must be positive",
            ));
        }
        let word_count = graph.word_count;
        let seed_words = multiply("seed array", word_count, worlds)?.max(1);
        let result_words = multiply(
            "result array",
            add("result stride", word_count, RECEIPT_WORDS)?,
            worlds,
        )?
        .max(1);
        // WGSL addresses arrays and products with u32 arithmetic.
        for (name, words) in [("seed array", seed_words), ("result array", result_words)] {
            address(name, words)?;
        }
        let seed_bytes = bytes(seed_words)?;
        let result_bytes = bytes(result_words)?;
        buffer_capacity("seeds", seed_bytes, device)?;
        buffer_capacity("results", result_bytes, device)?;
        if UNIFORM_BYTES > device.max_uniform_buffer_binding_size
            || UNIFORM_BYTES > device.max_buffer_size
        {
            return Err(GpuError::new(
                GpuErrorKind::Capacity,
                "device cannot bind the 32-byte parameter buffer",
            ));
        }
        let transport_bytes = [UNIFORM_BYTES, seed_bytes, result_bytes, result_bytes]
            .into_iter()
            .try_fold(0u64, add_bytes)?;
        let accounted_bytes =
            accounted_bytes(graph, worlds, seed_bytes, transport_bytes, upload_graph)?;
        if accounted_bytes > budget.max_batch_bytes {
            return Err(GpuError::new(
                GpuErrorKind::Capacity,
                format!(
                    "batch requests {accounted_bytes} accounted bytes, limit is {}",
                    budget.max_batch_bytes
                ),
            ));
        }
        Ok(Self {
            epoch,
            atom_count: graph.atom_count,
            word_count,
            rule_count: graph.rule_count,
            world_count: candidate_count,
            seed_words,
            seed_bytes,
            result_bytes,
            result_words,
            transport_bytes,
            accounted_bytes,
        })
    }
}

fn accounted_bytes(
    graph: &GraphPlan,
    worlds: usize,
    seed_bytes: u64,
    transport_bytes: u64,
    upload_graph: bool,
) -> Result<u64, GpuError> {
    // All active resident GPU buffers count. A new graph additionally
    // requires host packing. Exact-shape transport replacement drops old handles
    // before allocating the next transport; wgpu/driver retirement is excluded.
    let returned_words = bytes(multiply("returned closures", worlds, graph.word_count)?)?;
    let result_metadata = u64::try_from(multiply(
        "result metadata",
        worlds,
        std::mem::size_of::<GpuCheck>(),
    )?)
    .map_err(|error| GpuError::new(GpuErrorKind::Capacity, error.to_string()))?;
    [
        graph.resident_bytes,
        if upload_graph {
            graph.resident_bytes
        } else {
            0
        },
        transport_bytes,
        UNIFORM_BYTES,
        seed_bytes,
        bytes(graph.word_count)?,
        returned_words,
        result_metadata,
    ]
    .into_iter()
    .try_fold(0u64, add_bytes)
}

pub(crate) struct PackedGraph {
    pub(crate) rules: Vec<u32>,
    pub(crate) antecedents: Vec<u32>,
    pub(crate) carrier: Vec<u32>,
}

impl PackedGraph {
    pub(crate) fn new(program: &GroundProgram, plan: &GraphPlan) -> Result<Self, GpuError> {
        let mut rules = reserved(plan.rule_words)?;
        let mut antecedents = reserved(plan.antecedent_words)?;
        for rule in program.rules() {
            let head = match rule.head() {
                Some(atom) => checked_atom(atom, plan.atom_count)?,
                None => u32::MAX,
            };
            rules.push(head);
            for list in [rule.positive(), rule.gate_true(), rule.gate_false()] {
                rules.push(address("antecedent offset", antecedents.len())?);
                rules.push(address("antecedent length", list.len())?);
                for &atom in list {
                    antecedents.push(checked_atom(atom, plan.atom_count)?);
                }
            }
            rules.push(0);
        }
        rules.resize(plan.rule_words, 0);
        antecedents.resize(plan.antecedent_words, 0);
        let mut carrier = reserved(plan.carrier_words)?;
        carrier.resize(plan.carrier_words, 0);
        for &atom in program.gate_atom_ids() {
            checked_atom(atom, plan.atom_count)?;
            let word = usize::try_from(atom / 32)
                .map_err(|error| GpuError::new(GpuErrorKind::Capacity, error.to_string()))?;
            carrier[word] |= 1 << (atom % 32);
        }
        Ok(Self {
            rules,
            antecedents,
            carrier,
        })
    }
}

pub(crate) struct PackedSeeds<'program> {
    pub(crate) params: [u32; 8],
    pub(crate) seeds: Vec<u32>,
    // Borrowed for the synchronous call, like the original GroundProgram.
    // This permits an independent projection check without a second mask allocation.
    gate_atoms: &'program [u32],
}

impl<'program> PackedSeeds<'program> {
    pub(crate) fn new(
        program: &'program GroundProgram,
        seeds: &[Seed],
        plan: &BatchPlan,
    ) -> Result<Self, GpuError> {
        let mut seed_words = reserved(plan.seed_words)?;
        for seed in seeds {
            let words = program
                .seed_words(seed)
                .map_err(|error| GpuError::new(GpuErrorKind::Seed, error.to_string()))?;
            validate_words(&words, plan.atom_count, plan.word_count)?;
            seed_words.extend_from_slice(&words);
        }
        seed_words.resize(plan.seed_words, 0);
        Ok(Self {
            params: [
                plan.atom_count,
                address("closure word count", plan.word_count)?,
                plan.rule_count,
                plan.world_count,
                plan.epoch.get(),
                0,
                0,
                0,
            ],
            seeds: seed_words,
            gate_atoms: program.gate_atom_ids(),
        })
    }
}

pub(crate) fn decode(
    words: &[u32],
    plan: &BatchPlan,
    packed: &PackedSeeds<'_>,
) -> Result<Vec<GpuCheck>, GpuError> {
    if words.len() != plan.result_words {
        return Err(GpuError::new(
            GpuErrorKind::Readback,
            "readback length differs from the dispatched shape",
        ));
    }
    let candidates = usize::try_from(plan.world_count)
        .map_err(|error| GpuError::new(GpuErrorKind::Readback, error.to_string()))?;
    let mut results = reserved(candidates)?;
    for (world, record) in words
        .chunks_exact(plan.word_count + RECEIPT_WORDS)
        .take(candidates)
        .enumerate()
    {
        if record[0] != plan.epoch.get()
            || usize::try_from(record[1]).ok() != Some(world)
            || record[3] != COMPLETION_MARKER
        {
            return Err(GpuError::new(
                GpuErrorKind::Readback,
                "static result submission, world or completion identity differs",
            ));
        }
        let status = record[2];
        if status & !3 != 0 {
            return Err(GpuError::new(
                GpuErrorKind::Readback,
                "device returned an unknown verdict bit",
            ));
        }
        let closure = &record[RECEIPT_WORDS..];
        validate_words(closure, plan.atom_count, plan.word_count)?;
        let seed = &packed.seeds[world * plan.word_count..][..plan.word_count];
        let mismatch = packed.gate_atoms.iter().any(|&atom| {
            let word = (atom / 32) as usize;
            let mask = 1 << (atom % 32);
            closure[word] & mask != seed[word] & mask
        });
        if mismatch != (status & 2 != 0) {
            return Err(GpuError::new(
                GpuErrorKind::Readback,
                "static gate-projection verdict disagrees with closure and seed",
            ));
        }
        let mut closure_words = reserved(plan.word_count)?;
        closure_words.extend_from_slice(closure);
        results.push(GpuCheck {
            closure_words,
            status,
        });
    }
    Ok(results)
}

fn validate_words(words: &[u32], atom_count: u32, word_count: usize) -> Result<(), GpuError> {
    if words.len() != word_count {
        return Err(GpuError::new(
            GpuErrorKind::Readback,
            "closure/seed word count is inconsistent",
        ));
    }
    let remainder = atom_count % 32;
    if remainder != 0
        && words
            .last()
            .is_some_and(|last| last & !((1u32 << remainder) - 1) != 0)
    {
        return Err(GpuError::new(
            GpuErrorKind::Readback,
            "bits beyond the atom carrier are nonzero",
        ));
    }
    Ok(())
}

fn checked_atom(atom: u32, atom_count: u32) -> Result<u32, GpuError> {
    if atom < atom_count {
        Ok(atom)
    } else {
        Err(GpuError::new(
            GpuErrorKind::Validation,
            "compiled rule references an atom outside its dense carrier",
        ))
    }
}

fn buffer_capacity(name: &str, size: u64, limits: &wgpu::Limits) -> Result<(), GpuError> {
    let limit = limits
        .max_buffer_size
        .min(limits.max_storage_buffer_binding_size)
        // This profile also confines byte offsets to u32, independently of
        // any larger buffer sizes a future adapter might advertise.
        .min(u64::from(u32::MAX));
    if size > limit {
        return Err(GpuError::new(
            GpuErrorKind::Capacity,
            format!("{name} needs {size} bytes, device binding limit is {limit}"),
        ));
    }
    Ok(())
}

fn capacity(name: &str, size: usize, limit: usize) -> Result<(), GpuError> {
    if size > limit {
        return Err(GpuError::new(
            GpuErrorKind::Capacity,
            format!("{name} needs {size}, limit is {limit}"),
        ));
    }
    Ok(())
}

fn address(name: &str, size: usize) -> Result<u32, GpuError> {
    u32::try_from(size).map_err(|_| {
        GpuError::new(
            GpuErrorKind::Capacity,
            format!("{name} exceeds u32 addressing"),
        )
    })
}

fn multiply(name: &str, left: usize, right: usize) -> Result<usize, GpuError> {
    left.checked_mul(right)
        .ok_or_else(|| GpuError::new(GpuErrorKind::Capacity, format!("{name} size overflow")))
}

fn add(name: &str, left: usize, right: usize) -> Result<usize, GpuError> {
    left.checked_add(right)
        .ok_or_else(|| GpuError::new(GpuErrorKind::Capacity, format!("{name} size overflow")))
}

fn add_bytes(left: u64, right: u64) -> Result<u64, GpuError> {
    left.checked_add(right)
        .ok_or_else(|| GpuError::new(GpuErrorKind::Capacity, "batch byte count overflow"))
}

fn bytes(words: usize) -> Result<u64, GpuError> {
    u64::try_from(words)
        .ok()
        .and_then(|count| count.checked_mul(WORD_BYTES))
        .ok_or_else(|| GpuError::new(GpuErrorKind::Capacity, "buffer byte count overflow"))
}

fn reserved<T>(capacity: usize) -> Result<Vec<T>, GpuError> {
    let mut out = Vec::new();
    out.try_reserve_exact(capacity)
        .map_err(|error| GpuError::new(GpuErrorKind::Allocation, error.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{BatchPlan, GraphPlan, PackedGraph, PackedSeeds};
    use crate::{GpuErrorKind, GpuLimits};
    use std::num::NonZeroU32;
    use zetesis_core::{
        AdmissionLimits, AtomPattern, GroundProgram, Predicate, Program, Seed, StaticLimits,
        Template,
    };

    fn nullary(name: &str) -> AtomPattern {
        AtomPattern::new(Predicate::new(name, 0).expect("signature"), vec![]).expect("pattern")
    }

    fn fixture() -> GroundProgram {
        let a = nullary("a");
        let b = nullary("b");
        let c = nullary("c");
        let program = Program::new(
            vec![
                Template::new(Some(a.clone()), vec![], vec![a.clone()], vec![], vec![]),
                Template::new(Some(b.clone()), vec![a], vec![], vec![c.clone()], vec![]),
                Template::new(None, vec![b], vec![c], vec![], vec![]),
            ],
            AdmissionLimits::default(),
        )
        .expect("admitted fixture");
        GroundProgram::compile(&program, StaticLimits::default())
            .expect("small explicit static graph")
    }

    #[test]
    fn packing_separates_positive_support_gates_and_constraints() {
        let graph = fixture();
        let empty = Seed::new(graph.program(), []).expect("empty seed");
        let selected = Seed::new(
            graph.program(),
            [graph.atoms()[0].clone(), graph.atoms()[2].clone()],
        )
        .expect("gate seed");
        let plan = BatchPlan::new(&graph, 2, GpuLimits::default(), &wgpu::Limits::default())
            .expect("fits");
        let graph_plan = GraphPlan::new(&graph, &wgpu::Limits::default()).expect("graph fits");
        let packed = PackedGraph::new(&graph, &graph_plan).expect("pack graph");
        let batch = PackedSeeds::new(&graph, &[empty, selected], &plan).expect("pack seeds");
        assert_eq!(batch.params, [3, 1, 3, 2, 1, 0, 0, 0]);
        assert_eq!(
            packed.rules,
            [
                0,
                0,
                0,
                0,
                1,
                1,
                0,
                0,
                1,
                1,
                1,
                2,
                0,
                2,
                1,
                0,
                u32::MAX,
                3,
                1,
                4,
                1,
                5,
                0,
                0
            ]
        );
        assert_eq!(packed.antecedents, [0, 0, 2, 1, 2]);
        assert_eq!(batch.seeds, [0, 5]);
        assert_eq!(packed.carrier, [5]);
    }

    #[test]
    fn budgets_and_program_identity_are_checked_before_dispatch() {
        let graph = fixture();
        let budget = GpuLimits {
            max_batch_bytes: 0,
            ..GpuLimits::default()
        };
        assert!(
            matches!(BatchPlan::new(&graph, 1, budget, &wgpu::Limits::default()), Err(error) if error.kind() == GpuErrorKind::Capacity)
        );
        let plan = BatchPlan::new(&graph, 1, GpuLimits::default(), &wgpu::Limits::default())
            .expect("fits");
        let other = fixture();
        let foreign = Seed::new(other.program(), []).expect("other program seed");
        assert!(
            matches!(PackedSeeds::new(&graph, &[foreign], &plan), Err(error) if error.kind() == GpuErrorKind::Seed)
        );
    }

    #[test]
    fn resident_identity_accepts_clones_and_recompilation_but_not_equal_new_programs() {
        let graph = fixture();
        let resident = GraphPlan::new(&graph, &wgpu::Limits::default()).expect("resident plan");
        assert!(resident.matches(&graph.clone()));
        let recompiled = GroundProgram::compile(graph.program(), StaticLimits::default())
            .expect("same immutable Program compiles deterministically");
        assert!(resident.matches(&recompiled));
        let independently_admitted = fixture();
        assert_eq!(graph.atoms(), independently_admitted.atoms());
        assert!(!resident.matches(&independently_admitted));
        // The identity owns a Program clone rather than retaining a raw address
        // to the caller's GroundProgram or depending on its stack lifetime.
        drop(graph);
        assert!(resident.matches(&recompiled));
    }

    #[test]
    fn resident_graph_counts_toward_hot_budgets_without_repacking_cost() {
        let graph = fixture();
        let device = wgpu::Limits::default();
        let resident = GraphPlan::new(&graph, &device).expect("resident plan");
        let limits = GpuLimits::default();
        let cold = BatchPlan::for_graph(&resident, 2, limits, &device, true, NonZeroU32::MIN)
            .expect("cold plan");
        let hot = BatchPlan::for_graph(&resident, 2, limits, &device, false, NonZeroU32::MIN)
            .expect("hot plan");
        assert_eq!(
            cold.accounted_bytes - hot.accounted_bytes,
            resident.resident_bytes
        );
        assert_eq!(cold.transport_bytes, hot.transport_bytes);
        assert!(hot.accounted_bytes >= resident.resident_bytes + hot.transport_bytes);
        let exact = GpuLimits {
            max_batch_bytes: hot.accounted_bytes,
            ..limits
        };
        assert!(BatchPlan::for_graph(&resident, 2, exact, &device, false, NonZeroU32::MIN).is_ok());
        assert!(matches!(
            BatchPlan::for_graph(&resident, 2, exact, &device, true, NonZeroU32::MIN),
            Err(error) if error.kind() == GpuErrorKind::Capacity
        ));
        let too_small = GpuLimits {
            max_batch_bytes: hot.accounted_bytes - 1,
            ..limits
        };
        assert!(matches!(
            BatchPlan::for_graph(&resident, 2, too_small, &device, false, NonZeroU32::MIN),
            Err(error) if error.kind() == GpuErrorKind::Capacity
        ));
        let smaller = BatchPlan::for_graph(&resident, 1, limits, &device, false, NonZeroU32::MIN)
            .expect("smaller plan");
        assert!(smaller.transport_bytes < hot.transport_bytes);
        assert!(smaller.accounted_bytes < hot.accounted_bytes);
    }

    #[test]
    fn device_dispatch_and_binding_limits_are_enforced_before_allocation() {
        let graph = fixture();
        let one_world = wgpu::Limits {
            max_compute_workgroups_per_dimension: 1,
            ..wgpu::Limits::default()
        };
        assert!(
            matches!(BatchPlan::new(&graph, 2, GpuLimits::default(), &one_world), Err(error) if error.kind() == GpuErrorKind::Capacity)
        );
        let small_binding = wgpu::Limits {
            max_storage_buffer_binding_size: 16,
            ..wgpu::Limits::default()
        };
        assert!(
            matches!(BatchPlan::new(&graph, 1, GpuLimits::default(), &small_binding), Err(error) if error.kind() == GpuErrorKind::Capacity)
        );
    }
}

#[cfg(test)]
#[path = "../tests/packing/budgets.rs"]
mod budget_contract_tests;

#[cfg(test)]
#[path = "../tests/packing/readback.rs"]
mod readback_contract_tests;
