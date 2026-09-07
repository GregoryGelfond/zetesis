use crate::BenchmarkError;
use clap::ValueEnum;
use zetesis_core::{
    AdmissionLimits, AtomPattern, GroundProgram, Predicate, Program, Seed, StaticLimits, Template,
};

/// A validated deterministic static program for repeatable regression measurements.
pub struct BenchmarkFixture {
    graph: GroundProgram,
}
impl BenchmarkFixture {
    /// Construct one of the shared benchmark families.
    ///
    /// # Errors
    /// Refuses zero or more than 4,088 consequences, or failed core/static admission.
    pub fn new(family: Family, consequences: usize) -> Result<Self, BenchmarkError> {
        if consequences == 0 || consequences > zetesis_wgpu::MAX_ATOMS - 8 {
            return Err(BenchmarkError::Dimensions);
        }
        Ok(Self {
            graph: fixture(family, consequences)?,
        })
    }
    /// The explicitly compiled program; construction is outside dispatch timings.
    #[must_use]
    pub fn graph(&self) -> &GroundProgram {
        &self.graph
    }
    /// Deterministic worlds with shifted truth patterns; sizes above 256 repeat.
    ///
    /// # Errors
    /// Refuses zero or more than 4,096 worlds.
    pub fn seeds(&self, count: usize, salt: usize) -> Result<Vec<Seed>, BenchmarkError> {
        if count == 0 || count > 4096 {
            return Err(BenchmarkError::Dimensions);
        }
        Ok(seeds(&self.graph, count, salt))
    }
}

/// Families expose both rule-order sensitivity and independent parallel work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Family {
    /// Consequences follow their support in the scan order.
    ForwardChain,
    /// Consequences precede their support, requiring repeated dense scans.
    ReverseChain,
    /// Independent gated consequences share a single positive root.
    Wide,
}
impl Family {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::ForwardChain => "forward-chain",
            Self::ReverseChain => "reverse-chain",
            Self::Wide => "wide",
        }
    }
}

fn atom(name: String) -> AtomPattern {
    AtomPattern::new(
        Predicate::new(name, 0).expect("generated name is nonempty"),
        Vec::new(),
    )
    .expect("nullary pattern")
}

pub(crate) fn fixture(
    family: Family,
    consequences: usize,
) -> Result<GroundProgram, BenchmarkError> {
    let gates: Vec<_> = (0..8).map(|index| atom(format!("g{index}"))).collect();
    let atoms: Vec<_> = (0..consequences)
        .map(|index| atom(format!("p{index:04}")))
        .collect();
    let mut templates: Vec<_> = gates
        .iter()
        .map(|gate| {
            Template::new(
                Some(gate.clone()),
                vec![],
                vec![gate.clone()],
                vec![],
                vec![],
            )
        })
        .collect();
    templates.push(Template::new(
        Some(atoms[0].clone()),
        vec![],
        vec![],
        vec![],
        vec![],
    ));
    let mut rules: Vec<_> = (1..consequences)
        .map(|index| {
            let (positive, gate) = match family {
                Family::ForwardChain | Family::ReverseChain => {
                    (atoms[index - 1].clone(), gates[0].clone())
                }
                Family::Wide => (atoms[0].clone(), gates[index % 8].clone()),
            };
            Template::new(
                Some(atoms[index].clone()),
                vec![positive],
                vec![gate],
                vec![],
                vec![],
            )
        })
        .collect();
    if family == Family::ReverseChain {
        rules.reverse();
    }
    templates.extend(rules);
    // Both accepted and constraint-rejected worlds are compared in larger batches.
    templates.push(Template::new(
        None,
        vec![atoms[consequences - 1].clone()],
        vec![gates[7].clone()],
        vec![],
        vec![],
    ));
    let program =
        Program::new(templates, AdmissionLimits::default()).map_err(BenchmarkError::Admission)?;
    GroundProgram::compile(
        &program,
        StaticLimits {
            max_atoms: consequences + 8,
            max_ground_rules: consequences + 9,
            max_substitutions: consequences + 9,
        },
    )
    .map_err(BenchmarkError::Static)
}

pub(crate) fn seeds(graph: &GroundProgram, count: usize, salt: usize) -> Vec<Seed> {
    let gates: Vec<_> = graph
        .program()
        .gate_atoms()
        .map(|atom| atom.expect("eight nullary gates"))
        .collect();
    (0..count)
        .map(|world| {
            // Repetition above 256 worlds is deliberate batch-size scaling, not a
            // claim of additional unique candidates. Salt changes content in-place.
            let mask = world.wrapping_add(salt) & 255;
            Seed::new(
                graph.program(),
                gates
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, atom)| atom.clone()),
            )
            .expect("fixture gates belong to their graph")
        })
        .collect()
}
