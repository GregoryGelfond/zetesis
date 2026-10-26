//! 2QBF by saturation over hard unsatisfiable cores, the parity, Tseitin and
//! pigeonhole workloads, each checked against a direct evaluation of every
//! existential assignment's core.
use super::{BTreeSet, Workload, WorkloadLimits, bundles, examples, root};
use std::collections::BTreeMap;
use zetesis_validation::performance::matrix::{AuthoredProgram, ConstantAmendment};

/// A clause: its literals, each a universal variable and whether it is positive.
type Clause = Vec<(u32, bool)>;

/// One workload's files and seals.
struct Fixture {
    directory: &'static str,
    entry: &'static str,
    source_sha256: &'static str,
    manifest_sha256: &'static str,
}

const PARITY: Fixture = Fixture {
    directory: "scalability/two-qbf-parity",
    entry: "two_qbf_parity.lp",
    source_sha256: "2c3a862dabf858f0f017083a3c6753cf97fb33a7d69092817a451b78f98ff3ea",
    manifest_sha256: "ff861bd1ecae264ea0fa0a47a7708f24a1fbab0903078301702b430d71b79412",
};
const TSEITIN: Fixture = Fixture {
    directory: "scalability/two-qbf-tseitin",
    entry: "two_qbf_tseitin.lp",
    source_sha256: "7c18fe0f648b6d74ba4979e691d5e73ecd2e4074365da199fdef0f8f0710ad5b",
    manifest_sha256: "bb956bc556cc4a3b9f41c22ac6fefcf81549a68e777cae45c560560ee7509595",
};
const PIGEONHOLE: Fixture = Fixture {
    directory: "scalability/two-qbf-pigeonhole",
    entry: "two_qbf_pigeonhole.lp",
    source_sha256: "4de3da43035196cf7f37e5ba17222ceba22d019e223eb6e682c450a8dd6a0c7f",
    manifest_sha256: "989acb743ef41942a5c75d6cb845c4ddc58df453d93b17d5316f85dc8ab47d17",
};

/// The core a workload builds, with its constants as the program names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Core {
    Parity { n: i64, seed: i64 },
    Tseitin { vertices: i64 },
    Pigeonhole { holes: i64 },
}

/// One instance: the existential side and the core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Instance {
    exists: i64,
    stable: i64,
    core: Core,
}

const PARITY_DEFAULT: Instance = Instance {
    exists: 2,
    stable: 1,
    core: Core::Parity { n: 6, seed: 1 },
};
const TSEITIN_DEFAULT: Instance = Instance {
    exists: 2,
    stable: 1,
    core: Core::Tseitin { vertices: 12 },
};
const PIGEONHOLE_DEFAULT: Instance = Instance {
    exists: 2,
    stable: 1,
    core: Core::Pigeonhole { holes: 4 },
};

/// Each workload's default and cases small enough to evaluate directly: no
/// stable assignment, several, three existential variables, and another core.
fn cases(default: Instance) -> [Instance; 5] {
    let other = match default.core {
        Core::Parity { .. } => Core::Parity { n: 5, seed: 2 },
        Core::Tseitin { .. } => default.core,
        Core::Pigeonhole { .. } => Core::Pigeonhole { holes: 3 },
    };
    [
        default,
        Instance {
            stable: 0,
            ..default
        },
        Instance {
            stable: 2,
            ..default
        },
        Instance {
            exists: 3,
            stable: 3,
            ..default
        },
        Instance {
            stable: 4,
            core: other,
            ..default
        },
    ]
}

/// The program's hash of the seed and an input; every intermediate value stays
/// below 2^31.
fn key(seed: i64, input: i64) -> i64 {
    let square = |value: i64, offset: i64| (value * value + offset) % 46_337;
    let start = (seed * 7919 + input * 104_729) % 46_337;
    square(square(square(start, 12_345), 54_321), 7)
}

/// The clause forbidding `pattern` on `variables`: on each variable, the literal
/// its value falsifies.
fn forbidding(variables: &[u32], pattern: &[bool]) -> Clause {
    variables
        .iter()
        .zip(pattern)
        .map(|(&variable, &value)| (variable, !value))
        .collect()
}

/// The patterns of three bits whose parity is `parity`, in the Tseitin program's
/// order: pattern 4·s1 + 2·s2 + s3 + 1.
fn patterns(parity: bool) -> Vec<[bool; 3]> {
    (0..8_u8)
        .map(|bits| [bits & 4 != 0, bits & 2 != 0, bits & 1 != 0])
        .filter(|pattern| pattern.iter().filter(|&&bit| bit).count() % 2 == usize::from(parity))
        .collect()
}

/// The committed graph on `vertices` vertices, read from the program's facts
/// `edge(N, E, U, V).`, in edge order.
fn graph(vertices: i64) -> Vec<(u32, u32, u32)> {
    let source =
        std::fs::read_to_string(root().join(TSEITIN.directory).join(TSEITIN.entry)).unwrap();
    let mut edges = source
        .split("edge(")
        .skip(1)
        .filter_map(|fact| {
            let fields = fact
                .split_once(").")?
                .0
                .split(',')
                .map(|field| field.trim().parse::<i64>());
            let fields = fields.collect::<Result<Vec<_>, _>>().ok()?;
            let [graph, edge, from, to] = fields[..] else {
                return None;
            };
            (graph == vertices).then(|| {
                let narrow = |value: i64| u32::try_from(value).unwrap();
                (narrow(edge), narrow(from), narrow(to))
            })
        })
        .collect::<Vec<_>>();
    edges.sort_unstable();
    assert_eq!(edges.len() * 2, usize::try_from(vertices * 3).unwrap());
    edges
}

impl Core {
    /// The core's universal variable count and its clauses, keyed by the
    /// program's clause identifiers.
    fn clauses(self) -> (u32, BTreeMap<i64, Clause>) {
        let mut clauses = BTreeMap::new();
        match self {
            Self::Parity { n, seed } => {
                let variable = |value: i64| u32::try_from(value).unwrap();
                let mut order = (1..=n)
                    .map(|input| (key(seed, input) * 32 + input, input))
                    .collect::<Vec<_>>();
                order.sort_unstable();
                let order = order
                    .into_iter()
                    .map(|(_, input)| input)
                    .collect::<Vec<_>>();
                // gate (chain, k): (acc, in, out), out = acc xor in.
                let mut gates = Vec::new();
                for k in 1..n {
                    let first = if k == 1 { 1 } else { n + k - 1 };
                    gates.push((1, k, first, k + 1, n + k));
                    let second = if k == 1 { order[0] } else { 2 * n + k - 2 };
                    gates.push((
                        2,
                        k,
                        second,
                        order[usize::try_from(k).unwrap()],
                        2 * n - 1 + k,
                    ));
                }
                // The program's odd patterns, odd(1..4, S1, S2, S3), in its order.
                let odd = [
                    [true, false, false],
                    [false, true, false],
                    [false, false, true],
                    [true, true, true],
                ];
                for (chain, k, acc, input, output) in gates {
                    for (index, pattern) in (1..).zip(odd) {
                        let identifier = ((chain - 1) * (n - 1) + k - 1) * 4 + index;
                        let variables = [variable(acc), variable(input), variable(output)];
                        clauses.insert(identifier, forbidding(&variables, &pattern));
                    }
                }
                let (first, second) = (variable(2 * n - 1), variable(3 * n - 2));
                clauses.insert(8 * (n - 1) + 1, vec![(first, true), (second, true)]);
                clauses.insert(8 * (n - 1) + 2, vec![(first, false), (second, false)]);
                (variable(3 * n - 2), clauses)
            }
            Self::Tseitin { vertices } => {
                let edges = graph(vertices);
                for vertex in 1..=u32::try_from(vertices).unwrap() {
                    let incident = edges
                        .iter()
                        .filter(|&&(_, from, to)| from == vertex || to == vertex)
                        .map(|&(edge, _, _)| edge)
                        .collect::<Vec<_>>();
                    // Vertex 1 has charge 1, every other vertex charge 0; a
                    // pattern is forbidden when its parity differs.
                    for (index, pattern) in (1..).zip(patterns(vertex != 1)) {
                        let identifier = i64::from(vertex - 1) * 4 + index;
                        clauses.insert(identifier, forbidding(&incident, &pattern));
                    }
                }
                (u32::try_from(edges.len()).unwrap(), clauses)
            }
            Self::Pigeonhole { holes } => {
                let variable =
                    |pigeon: i64, hole: i64| u32::try_from((pigeon - 1) * holes + hole).unwrap();
                for pigeon in 1..=holes + 1 {
                    clauses.insert(
                        pigeon,
                        (1..=holes)
                            .map(|hole| (variable(pigeon, hole), true))
                            .collect(),
                    );
                }
                for hole in 1..=holes {
                    for first in 1..=holes + 1 {
                        for second in first + 1..=holes + 1 {
                            let identifier = holes
                                + 1
                                + ((hole - 1) * (holes + 1) + first - 1) * (holes + 1)
                                + second;
                            let literals = vec![
                                (variable(first, hole), false),
                                (variable(second, hole), false),
                            ];
                            clauses.insert(identifier, literals);
                        }
                    }
                }
                (u32::try_from((holes + 1) * holes).unwrap(), clauses)
            }
        }
    }
}

/// Whether some assignment of `variables` universal variables satisfies every
/// clause, by trying them all.
fn satisfiable(variables: u32, clauses: &[&Clause]) -> bool {
    assert!(
        variables <= 20,
        "{variables} variables are too many to enumerate"
    );
    let masks = clauses
        .iter()
        .map(|clause| {
            clause
                .iter()
                .fold((0_u32, 0_u32), |(positive, negative), &(variable, sign)| {
                    let bit = 1 << (variable - 1);
                    if sign {
                        (positive | bit, negative)
                    } else {
                        (positive, negative | bit)
                    }
                })
        })
        .collect::<Vec<_>>();
    (0..1_u32 << variables).any(|assignment| {
        masks
            .iter()
            .all(|&(positive, negative)| assignment & positive != 0 || !assignment & negative != 0)
    })
}

impl Instance {
    /// The complete answer family: every existential assignment whose core no
    /// universal assignment satisfies, displayed as the program shows it. An
    /// assignment beyond the first `stable` drops the clause of rank
    /// `A - stable + 1` from its core.
    fn family(self) -> BTreeSet<Vec<String>> {
        let (variables, clauses) = self.core.clauses();
        let ordered = clauses.values().collect::<Vec<_>>();
        (0..1_i64 << self.exists)
            .filter(|&assignment| {
                let core = ordered
                    .iter()
                    .enumerate()
                    .filter(|&(rank, _)| {
                        assignment < self.stable
                            || i64::try_from(rank).unwrap() != assignment - self.stable
                    })
                    .map(|(_, &clause)| clause)
                    .collect::<Vec<_>>();
                !satisfiable(variables, &core)
            })
            .map(|assignment| {
                let mut display = (1..=self.exists)
                    .map(|variable| {
                        let name = if assignment & (1 << (variable - 1)) != 0 {
                            "x"
                        } else {
                            "nx"
                        };
                        format!("{name}({variable})")
                    })
                    .collect::<Vec<_>>();
                display.sort();
                display
            })
            .collect()
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
    fn constants(self) -> Vec<(&'static str, i64)> {
        let mut constants = vec![("exists", self.exists), ("stable", self.stable)];
        match self.core {
            Core::Parity { n, seed } => constants.extend([("n", n), ("seed", seed)]),
            Core::Tseitin { vertices } => constants.push(("vertices", vertices)),
            Core::Pigeonhole { holes } => constants.push(("holes", holes)),
        }
        constants
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

/// The three workloads with their defaults.
const FIXTURES: [(&Fixture, Instance); 3] = [
    (&PARITY, PARITY_DEFAULT),
    (&TSEITIN, TSEITIN_DEFAULT),
    (&PIGEONHOLE, PIGEONHOLE_DEFAULT),
];

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
fn two_qbf_cores_seal_their_sources_and_manifests() {
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
fn two_qbf_core_default_contracts_are_their_complete_families() {
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
fn two_qbf_core_answer_sets_are_the_first_stable_assignments() {
    // Every whole core is unsatisfiable and every core less one clause is
    // satisfiable, so each instance's family is its first `stable`
    // assignments, the construction the workloads' comments state.
    for (_, default) in FIXTURES {
        for case in cases(default) {
            let count = 1_i64 << case.exists;
            assert_eq!(
                case.family().len(),
                usize::try_from(case.stable.min(count)).unwrap(),
                "{case:?}"
            );
        }
    }
}

#[test]
fn two_qbf_core_cases_include_true_and_false_formulas() {
    // The oracle gate then checks both outcomes of the outer quantifier.
    for (_, default) in FIXTURES {
        let counts = cases(default).map(|case| case.family().len());
        assert!(counts.contains(&0), "{counts:?}");
        assert!(counts.iter().any(|&count| count > 0), "{counts:?}");
    }
}

#[test]
fn two_qbf_core_constant_edits_retain_checked_provenance() {
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
        let stale = ConstantAmendment {
            source_path: fixture.entry,
            name: "stable",
            expected: 2,
            replacement: 0,
        };
        assert!(Workload::authored(program, &[stale], WorkloadLimits::default()).is_err());
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "requires clingo: every core case has its direct evaluation's complete family"]
fn two_qbf_core_complete_families_satisfy_their_cores() {
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
