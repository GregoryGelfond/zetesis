//! Workloads whose constraints carry algebraic structure: Tseitin's parity
//! formula, two parity chains, the pigeonhole principle in two encodings and a
//! satisfiable mixture of parity constraints and random clauses. Each is
//! checked against a direct evaluation of its constraints over every assignment
//! of its atoms.
use super::{BTreeSet, Workload, WorkloadLimits, bundles, examples, root};
use zetesis_validation::performance::matrix::{AuthoredProgram, ConstantAmendment};

/// One workload's files and seals.
struct Fixture {
    directory: &'static str,
    entry: &'static str,
    source_sha256: &'static str,
    manifest_sha256: &'static str,
}

const TSEITIN: Fixture = Fixture {
    directory: "scalability/parity-tseitin",
    entry: "parity_tseitin.lp",
    source_sha256: "4c22676741999506ad9074b7ee85c43d2e39d30cdd34ba44e5348a44abb7b142",
    manifest_sha256: "f5c0da37f6ba41e76bd3de2ebf812db73b84bafab94b63f6017e6f4e624292e3",
};
const CHAIN: Fixture = Fixture {
    directory: "scalability/parity-chain",
    entry: "parity_chain.lp",
    source_sha256: "3be3da465bf6270d2d01dd87b1bdb2b6c3196024daee692152cefbda54ffbfda",
    manifest_sha256: "2d39bb62a544ad3a7a22ff9a7b5b342c8b719af35e343298c8a41e8dd5a97e7b",
};
const PIGEONHOLE: Fixture = Fixture {
    directory: "scalability/pigeonhole-forms",
    entry: "pigeonhole_forms.lp",
    source_sha256: "fb0c2956b50f80b5928dadf945c6f296c7d63a0d044cfc805d406e5cd9df022a",
    manifest_sha256: "04d204df02ec95550af2acb5d0a8aace790b6ab082ed50882093df49a280e05f",
};
const MIXED: Fixture = Fixture {
    directory: "scalability/parity-mixed",
    entry: "parity_mixed.lp",
    source_sha256: "850168bb72c8068b6f9e067400a688293df88b5187409093c1be88754cbd80fe",
    manifest_sha256: "9f0dbfdd9166b93648441d00e8cd50832a64d108578948bfb70e308d3ee86722",
};

/// One instance, with its constants as the program names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Instance {
    Tseitin { p: i64, seed: i64, odd: i64 },
    Chain { p: i64, seed: i64, parity: i64 },
    Pigeonhole { holes: i64, extra: i64, form: i64 },
    Mixed { p: i64, clauses: i64, seed: i64 },
}

const TSEITIN_DEFAULT: Instance = Instance::Tseitin {
    p: 5,
    seed: 1,
    odd: 1,
};
const CHAIN_DEFAULT: Instance = Instance::Chain {
    p: 5,
    seed: 1,
    parity: 1,
};
const PIGEONHOLE_DEFAULT: Instance = Instance::Pigeonhole {
    holes: 4,
    extra: 1,
    form: 1,
};
const MIXED_DEFAULT: Instance = Instance::Mixed {
    p: 5,
    clauses: 8,
    seed: 1,
};

/// The four workloads with their defaults.
const FIXTURES: [(&Fixture, Instance); 4] = [
    (&TSEITIN, TSEITIN_DEFAULT),
    (&CHAIN, CHAIN_DEFAULT),
    (&PIGEONHOLE, PIGEONHOLE_DEFAULT),
    (&MIXED, MIXED_DEFAULT),
];

/// Each workload's default and cases small enough to evaluate directly: the
/// other outcome, a smaller instance, another seed or encoding.
fn cases(default: Instance) -> Vec<Instance> {
    match default {
        Instance::Tseitin { .. } => vec![
            default,
            Instance::Tseitin {
                p: 5,
                seed: 1,
                odd: 0,
            },
            Instance::Tseitin {
                p: 2,
                seed: 1,
                odd: 1,
            },
            Instance::Tseitin {
                p: 3,
                seed: 1,
                odd: 0,
            },
            Instance::Tseitin {
                p: 5,
                seed: 2,
                odd: 1,
            },
        ],
        Instance::Chain { .. } => vec![
            default,
            Instance::Chain {
                p: 5,
                seed: 1,
                parity: 0,
            },
            Instance::Chain {
                p: 2,
                seed: 1,
                parity: 1,
            },
            Instance::Chain {
                p: 3,
                seed: 1,
                parity: 0,
            },
            Instance::Chain {
                p: 5,
                seed: 2,
                parity: 1,
            },
        ],
        Instance::Pigeonhole { .. } => vec![
            default,
            Instance::Pigeonhole {
                holes: 4,
                extra: 1,
                form: 2,
            },
            Instance::Pigeonhole {
                holes: 4,
                extra: 0,
                form: 1,
            },
            Instance::Pigeonhole {
                holes: 4,
                extra: 0,
                form: 2,
            },
            Instance::Pigeonhole {
                holes: 3,
                extra: 1,
                form: 2,
            },
        ],
        Instance::Mixed { .. } => vec![
            default,
            Instance::Mixed {
                p: 5,
                clauses: 0,
                seed: 1,
            },
            Instance::Mixed {
                p: 5,
                clauses: 8,
                seed: 2,
            },
            Instance::Mixed {
                p: 3,
                clauses: 4,
                seed: 1,
            },
            Instance::Mixed {
                p: 5,
                clauses: 20,
                seed: 1,
            },
        ],
    }
}

/// The programs' permutation of `0..p`: `I -> (I + seed)^3 + 7 mod p`.
fn permutation(p: i64, seed: i64, index: i64) -> i64 {
    let base = (index + seed) % p;
    let square = base * base % p;
    (square * base + 7) % p
}

/// The graph on `2p` vertices, in edge order: a cycle through every vertex,
/// edges `1..=2p`, then the matching joining `I` to `p + pi(I)`.
fn graph(p: i64, seed: i64) -> Vec<(i64, i64, i64)> {
    let cycle = (0..2 * p).map(|from| (from + 1, from, (from + 1) % (2 * p)));
    let matching = (0..p).map(|from| (2 * p + 1 + from, from, p + permutation(p, seed, from)));
    cycle.chain(matching).collect()
}

/// Each vertex's increasing triples of distinct incident edges, as the
/// programs' `edges/4` derives them.
fn triples(p: i64, seed: i64) -> Vec<(i64, [i64; 3])> {
    let edges = graph(p, seed);
    let mut found = Vec::new();
    for vertex in 0..2 * p {
        let incident = edges
            .iter()
            .filter(|&&(_, from, to)| from == vertex || to == vertex)
            .map(|&(edge, _, _)| edge)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        for (first, &a) in incident.iter().enumerate() {
            for (second, &b) in incident.iter().enumerate().skip(first + 1) {
                for &c in &incident[second + 1..] {
                    found.push((vertex, [a, b, c]));
                }
            }
        }
    }
    found
}

/// The program's hash of the seed, a clause and a literal slot; every
/// intermediate value stays below 2^31.
fn key(seed: i64, clause: i64, slot: i64) -> i64 {
    let square = |value: i64, offset: i64| (value * value + offset) % 46_337;
    let start = (seed * 7919 + clause * 104_729 + slot * 1_299_709) % 46_337;
    square(square(square(start, 12_345), 54_321), 7)
}

/// The mixed workload's random clauses: three distinct edges and the value each
/// literal requires.
fn clauses(p: i64, count: i64, seed: i64) -> Vec<[(i64, bool); 3]> {
    (1..=count)
        .map(|clause| {
            let [first, second, third] = [1, 2, 3].map(|slot| key(seed, clause, slot));
            let edges = 3 * p;
            let start = first % edges;
            let step = 1 + second % (edges - 2);
            let further = 1 + third % (edges - 1 - step);
            let sign = |value: i64| (value / 61) % 2 == 1;
            [
                (start + 1, sign(first)),
                ((start + step) % edges + 1, sign(second)),
                ((start + step + further) % edges + 1, sign(third)),
            ]
        })
        .collect()
}

/// The constraints a program states, over atoms numbered from 1, in their
/// mathematical form.
#[derive(Default)]
struct Constraints {
    /// Atoms whose truth values must have the given exclusive-or.
    parities: Vec<(Vec<i64>, bool)>,
    /// Literals, each an atom and the value it requires, one of which must hold.
    clauses: Vec<Vec<(i64, bool)>>,
    /// Atoms with inclusive bounds on how many of them hold.
    counts: Vec<(Vec<i64>, usize, usize)>,
}

impl Constraints {
    /// Whether an assignment, bit `i` for atom `i + 1`, satisfies every
    /// constraint.
    fn allow(&self, assignment: u32) -> bool {
        let holds = |atom: i64| assignment & (1 << (atom - 1)) != 0;
        self.parities.iter().all(|(atoms, parity)| {
            atoms.iter().fold(false, |sum, &atom| sum ^ holds(atom)) == *parity
        }) && self
            .clauses
            .iter()
            .all(|literals| literals.iter().any(|&(atom, value)| holds(atom) == value))
            && self.counts.iter().all(|(atoms, lower, upper)| {
                let count = atoms.iter().filter(|&&atom| holds(atom)).count();
                (*lower..=*upper).contains(&count)
            })
    }
}

impl Instance {
    /// The number of atoms the evaluation assigns.
    fn atoms(self) -> u32 {
        let count = match self {
            Self::Tseitin { p, .. } | Self::Mixed { p, .. } => 3 * p,
            Self::Chain { p, .. } => 3 * p - 2,
            Self::Pigeonhole { holes, extra, .. } => (holes + extra) * holes,
        };
        u32::try_from(count).unwrap()
    }

    /// The program's constraints. Tseitin's vertices and the mixed workload's
    /// vertices are parities of their three edges; the chains' gates are
    /// parities of their three atoms; every pigeon occupies at least one hole
    /// and every hole holds at most one pigeon, in either encoding.
    fn constraints(self) -> Constraints {
        let mut constraints = Constraints::default();
        match self {
            Self::Tseitin { p, seed, odd } => {
                for (vertex, edges) in triples(p, seed) {
                    let charge = vertex == 0 && odd == 1;
                    constraints.parities.push((edges.to_vec(), charge));
                }
            }
            Self::Chain { p, seed, parity } => {
                let input = |chain: i64, step: i64| {
                    if chain == 1 {
                        step
                    } else {
                        permutation(p, seed, step - 1) + 1
                    }
                };
                let result = |chain: i64, step: i64| match (chain, step) {
                    (_, 1) => input(chain, 1),
                    (1, _) => p + step - 1,
                    _ => 2 * p + step - 2,
                };
                for chain in 1..=2 {
                    for step in 2..=p {
                        let gate = vec![
                            result(chain, step - 1),
                            input(chain, step),
                            result(chain, step),
                        ];
                        constraints.parities.push((gate, false));
                    }
                }
                let last = vec![result(1, p), result(2, p)];
                constraints.parities.push((last, parity == 1));
            }
            Self::Pigeonhole { holes, extra, .. } => {
                let atom = |pigeon: i64, hole: i64| (pigeon - 1) * holes + hole;
                let pigeons = holes + extra;
                let places = usize::try_from(holes).unwrap();
                for pigeon in 1..=pigeons {
                    let row = (1..=holes).map(|hole| atom(pigeon, hole)).collect();
                    constraints.counts.push((row, 1, places));
                }
                for hole in 1..=holes {
                    let column = (1..=pigeons).map(|pigeon| atom(pigeon, hole)).collect();
                    constraints.counts.push((column, 0, 1));
                }
            }
            Self::Mixed {
                p,
                clauses: count,
                seed,
            } => {
                for (_, edges) in triples(p, seed) {
                    constraints.parities.push((edges.to_vec(), false));
                }
                for clause in clauses(p, count, seed) {
                    constraints.clauses.push(clause.to_vec());
                }
            }
        }
        constraints
    }

    /// The atoms an answer set displays, sorted as displays are.
    fn display(self, assignment: u32) -> Vec<String> {
        let holds = |atom: i64| assignment & (1 << (atom - 1)) != 0;
        let mut display = match self {
            Self::Tseitin { p, .. } | Self::Mixed { p, .. } => (1..=3 * p)
                .filter(|&edge| holds(edge))
                .map(|edge| format!("on({edge})"))
                .collect::<Vec<_>>(),
            Self::Chain { p, .. } => (1..=p)
                .filter(|&input| holds(input))
                .map(|input| format!("on({input})"))
                .collect(),
            Self::Pigeonhole { holes, extra, .. } => (1..=holes + extra)
                .flat_map(|pigeon| (1..=holes).map(move |hole| (pigeon, hole)))
                .filter(|&(pigeon, hole)| holds((pigeon - 1) * holes + hole))
                .map(|(pigeon, hole)| format!("in({pigeon},{hole})"))
                .collect(),
        };
        display.sort();
        display
    }

    /// The complete answer family, by evaluating every assignment of the atoms.
    fn family(self) -> BTreeSet<Vec<String>> {
        assert!(
            self.atoms() <= 20,
            "{self:?} has too many atoms to enumerate"
        );
        let constraints = self.constraints();
        (0..1_u32 << self.atoms())
            .filter(|&assignment| constraints.allow(assignment))
            .map(|assignment| self.display(assignment))
            .collect()
    }

    /// The number of answer sets the construction states, where it states one.
    fn constructed_count(self) -> Option<usize> {
        let power = |exponent: i64| 1_usize << usize::try_from(exponent).unwrap();
        match self {
            Self::Tseitin { odd: 1, .. }
            | Self::Chain { parity: 1, .. }
            | Self::Pigeonhole { extra: 1, .. } => Some(0),
            Self::Tseitin { p, .. } => Some(power(p + 1)),
            Self::Chain { p, .. } => Some(power(p)),
            Self::Pigeonhole { holes, .. } => Some((1..=usize::try_from(holes).unwrap()).product()),
            Self::Mixed { .. } => None,
        }
    }

    /// The complete display contract the family determines.
    fn expected(self) -> examples::Contract {
        let family = self.family();
        let satisfiability = if family.is_empty() { "unsat" } else { "sat" };
        serde_json::from_value(serde_json::json!({
            "satisfiability": satisfiability, "family": "all",
            "model_count": family.len(), "cost": null,
            "witnesses": family, "required_symbols": [], "notes": []
        }))
        .unwrap()
    }

    /// The program's constants and values for this instance.
    fn constants(self) -> [(&'static str, i64); 3] {
        match self {
            Self::Tseitin { p, seed, odd } => [("p", p), ("seed", seed), ("odd", odd)],
            Self::Chain { p, seed, parity } => [("p", p), ("seed", seed), ("parity", parity)],
            Self::Pigeonhole { holes, extra, form } => {
                [("holes", holes), ("extra", extra), ("form", form)]
            }
            Self::Mixed { p, clauses, seed } => [("p", p), ("clauses", clauses), ("seed", seed)],
        }
    }

    /// The `#const` edits that turn `default` into this instance.
    fn edits(self, default: Self, entry: &'static str) -> Vec<ConstantAmendment<'static>> {
        default
            .constants()
            .into_iter()
            .zip(self.constants())
            .filter(|((_, before), (_, after))| before != after)
            .map(|((name, expected), (_, replacement))| ConstantAmendment {
                source_path: entry,
                name,
                expected: i32::try_from(expected).unwrap(),
                replacement: i32::try_from(replacement).unwrap(),
            })
            .collect()
    }
}

fn workload(fixture: &Fixture) -> Workload {
    let mut workloads = bundles::load(
        &root().join(fixture.directory).join("manifest.json"),
        WorkloadLimits::default(),
    )
    .unwrap();
    assert_eq!(workloads.len(), 1);
    workloads.pop().unwrap()
}

#[test]
fn algebraic_classes_seal_their_sources_and_manifests() {
    for (fixture, _) in FIXTURES {
        let workload = workload(fixture);
        assert!(workload.is_bundle());
        assert_eq!(workload.entry(), fixture.entry);
        let record = serde_json::to_value(&workload).unwrap();
        assert_eq!(
            record["bundle"]["manifest"]["sha256"],
            fixture.manifest_sha256
        );
        assert_eq!(
            record["bundle"]["files"][0]["sha256"],
            fixture.source_sha256
        );
    }
}

#[test]
fn algebraic_class_default_contracts_are_their_complete_families() {
    for (fixture, default) in FIXTURES {
        let contract = workload(fixture).contract().unwrap().clone();
        let expected = default.expected();
        assert_eq!(contract.satisfiability(), expected.satisfiability());
        assert_eq!(contract.family(), examples::Family::All);
        assert_eq!(contract.model_count(), expected.model_count());
        assert_eq!(contract.cost(), None);
        assert_eq!(
            contract.witnesses().iter().collect::<BTreeSet<_>>(),
            expected.witnesses().iter().collect::<BTreeSet<_>>(),
            "{}",
            fixture.entry
        );
    }
}

#[test]
fn algebraic_class_families_have_their_constructed_counts() {
    // Odd charge, differing chains and an extra pigeon leave nothing; otherwise
    // the even subgraphs, the input assignments and the placements remain. The
    // workloads' comments state these counts.
    for (_, default) in FIXTURES {
        for case in cases(default) {
            if let Some(count) = case.constructed_count() {
                assert_eq!(case.family().len(), count, "{case:?}");
            }
        }
    }
}

#[test]
fn algebraic_class_cases_include_both_outcomes() {
    // The oracle gate then checks unsatisfiable and satisfiable instances of each
    // structured workload; the mixed workload is satisfiable by design.
    for (_, default) in FIXTURES {
        let counts = cases(default)
            .into_iter()
            .map(|case| case.family().len())
            .collect::<Vec<_>>();
        assert!(counts.iter().any(|&count| count > 0), "{counts:?}");
        if !matches!(default, Instance::Mixed { .. }) {
            assert!(counts.contains(&0), "{counts:?}");
        }
    }
}

#[test]
fn algebraic_class_constant_edits_retain_checked_provenance() {
    for (fixture, default) in FIXTURES {
        let original = workload(fixture);
        let directory = root().join(fixture.directory);
        let program = AuthoredProgram {
            root: &directory,
            entry: fixture.entry,
            sha256: fixture.source_sha256,
            contract: original.contract().unwrap(),
        };
        let mut identities = BTreeSet::new();
        for case in cases(default) {
            let edits = case.edits(default, fixture.entry);
            let amended = Workload::authored(program, &edits, WorkloadLimits::default()).unwrap();
            let record = serde_json::to_value(&amended).unwrap();
            let recorded = record["sources"][0]["edits"].as_array().unwrap();
            assert_eq!(recorded.len(), edits.len());
            assert_eq!(amended.is_amended(), case != default);
            assert_eq!(amended.contract().is_none(), amended.is_amended());
            assert!(identities.insert(amended.identity().to_owned()), "{case:?}");
        }
        let (name, value) = default.constants()[0];
        let stale = ConstantAmendment {
            source_path: fixture.entry,
            name,
            expected: i32::try_from(value + 1).unwrap(),
            replacement: i32::try_from(value).unwrap(),
        };
        assert!(Workload::authored(program, &[stale], WorkloadLimits::default()).is_err());
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "requires clingo: every algebraic-class case has its direct evaluation's complete family"]
fn algebraic_class_complete_families_satisfy_their_constraints() {
    use std::time::Duration;
    use zetesis_clingo_support as oracle;
    for (fixture, default) in FIXTURES {
        for case in cases(default) {
            let constants = case
                .edits(default, fixture.entry)
                .iter()
                .map(|edit| format!("{}={}", edit.name, edit.replacement))
                .collect::<Vec<_>>();
            let arguments = ["--models=0", "--outf=2"]
                .into_iter()
                .map(str::to_owned)
                .chain(
                    constants
                        .iter()
                        .flat_map(|constant| ["--const".to_owned(), constant.clone()]),
                )
                .chain([fixture.entry.to_owned()])
                .collect::<Vec<_>>();
            let run = oracle::run_in(
                &root().join(fixture.directory),
                arguments.iter().map(String::as_str),
                &oracle::DECIDED,
                oracle::Limits {
                    timeout: Duration::from_mins(1),
                    max_output_bytes: 8 * 1024 * 1024,
                },
            );
            case.expected()
                .check(&oracle::answers(&run))
                .unwrap_or_else(|mismatch| panic!("{case:?}: {mismatch:?}"));
        }
    }
}
