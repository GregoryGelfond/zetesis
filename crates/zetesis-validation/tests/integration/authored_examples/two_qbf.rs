//! 2QBF by saturation, the corpus's first disjunctive program, checked against a
//! direct evaluation of its quantified formula.
use super::{BTreeSet, Workload, WorkloadLimits, bundles, examples, root};
use zetesis_validation::performance::matrix::{AuthoredProgram, ConstantAmendment};

const ENTRY: &str = "two_qbf.lp";
const SOURCE_SHA256: &str = "c9dc3cf1c650fae86a0d7fa5827c531a8b415eb3dfdcd200cfef47a8dbed7e44";
const MANIFEST_SHA256: &str = "7e1cb2db1cdd5cdb321ffdcb2d08fea67f48f601be088afeb9211e17855ee4a5";

/// One instance's constants, as the program names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Instance {
    exists: i64,
    forall: i64,
    width: i64,
    terms: i64,
    seed: i64,
}

const DEFAULT: Instance = Instance {
    exists: 3,
    forall: 8,
    width: 3,
    terms: 68,
    seed: 1,
};

/// The default, true and false formulas at the default size, a smaller
/// existential side, and a wider universal side.
const CASES: [Instance; 5] = [
    DEFAULT,
    Instance {
        terms: 52,
        ..DEFAULT
    },
    Instance { seed: 2, ..DEFAULT },
    Instance {
        exists: 2,
        terms: 40,
        ..DEFAULT
    },
    Instance {
        forall: 10,
        terms: 85,
        ..DEFAULT
    },
];

/// A literal: its variable, and whether it is positive.
type Literal = (i64, bool);

impl Instance {
    /// The program's hash, evaluated as the program does; every intermediate
    /// value stays below 2^31.
    fn key(self, term: i64, slot: i64) -> i64 {
        let square = |value: i64, offset: i64| (value * value + offset) % 46_337;
        let start = (self.seed * 7919 + term * 104_729 + slot * 1_299_709) % 46_337;
        square(square(square(start, 12_345), 54_321), 7)
    }

    /// Each term's existential literal and universal literals: the `width`
    /// universal variables of smallest rank, the key with the index appended.
    fn formula(self) -> Vec<(Literal, Vec<Literal>)> {
        (1..=self.terms)
            .map(|term| {
                let key = self.key(term, 0);
                let existential = (key % self.exists + 1, (key / 61) % 2 == 1);
                let mut ranked = (1..=self.forall)
                    .map(|variable| (self.key(term, variable) * 32 + variable, variable))
                    .collect::<Vec<_>>();
                ranked.sort_unstable();
                let universal = ranked
                    .iter()
                    .take(usize::try_from(self.width).unwrap())
                    .map(|&(_, variable)| (variable, (self.key(term, variable) / 61) % 2 == 1))
                    .collect();
                (existential, universal)
            })
            .collect()
    }

    /// The complete answer family: every existential assignment under which each
    /// universal assignment satisfies some term, displayed as the program shows it.
    fn family(self) -> BTreeSet<Vec<String>> {
        let formula = self.formula();
        let value = |assignment: u32, variable: i64| assignment & (1 << (variable - 1)) != 0;
        (0..1_u32 << self.exists)
            .filter(|&existential| {
                (0..1_u32 << self.forall).all(|universal| {
                    formula.iter().any(|&((variable, positive), ref literals)| {
                        value(existential, variable) == positive
                            && literals
                                .iter()
                                .all(|&(variable, positive)| value(universal, variable) == positive)
                    })
                })
            })
            .map(|existential| {
                let mut display = (1..=self.exists)
                    .map(|variable| {
                        let name = if value(existential, variable) {
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

    /// The `#const` edits that turn the default into this instance.
    fn edits(self) -> Vec<ConstantAmendment<'static>> {
        [
            ("exists", DEFAULT.exists, self.exists),
            ("forall", DEFAULT.forall, self.forall),
            ("width", DEFAULT.width, self.width),
            ("terms", DEFAULT.terms, self.terms),
            ("seed", DEFAULT.seed, self.seed),
        ]
        .into_iter()
        .filter(|(_, before, after)| before != after)
        .map(|(name, expected, replacement)| ConstantAmendment {
            source_path: ENTRY,
            name,
            expected: i32::try_from(expected).unwrap(),
            replacement: i32::try_from(replacement).unwrap(),
        })
        .collect()
    }
}

fn workload() -> Workload {
    let mut workloads = bundles::load(
        &root().join("scalability/two-qbf/manifest.json"),
        WorkloadLimits::default(),
    )
    .unwrap();
    assert_eq!(workloads.len(), 1);
    workloads.pop().unwrap()
}

#[test]
fn two_qbf_seals_its_source_and_manifest() {
    let workload = workload();
    assert!(workload.is_bundle());
    assert_eq!(workload.entry(), ENTRY);
    let record = serde_json::to_value(&workload).unwrap();
    assert_eq!(record["bundle"]["manifest"]["sha256"], MANIFEST_SHA256);
    assert_eq!(record["bundle"]["files"][0]["sha256"], SOURCE_SHA256);
}

#[test]
fn two_qbf_default_contract_is_the_formula_s_complete_family() {
    let contract = workload().contract().unwrap().clone();
    let expected = DEFAULT.expected();
    assert_eq!(contract.satisfiability(), expected.satisfiability());
    assert_eq!(contract.family(), examples::Family::All);
    assert_eq!(contract.model_count(), expected.model_count());
    assert_eq!(contract.cost(), None);
    assert_eq!(
        contract.witnesses().iter().collect::<BTreeSet<_>>(),
        expected.witnesses().iter().collect::<BTreeSet<_>>()
    );
}

#[test]
fn two_qbf_cases_include_true_and_false_formulas() {
    // The oracle gate then checks both outcomes of the outer quantifier.
    let counts = CASES.map(|case| case.family().len());
    assert!(counts.contains(&0), "{counts:?}");
    assert!(counts.iter().any(|&count| count > 0), "{counts:?}");
}

#[test]
fn two_qbf_constant_edits_retain_checked_provenance() {
    let original = workload();
    let directory = root().join("scalability/two-qbf");
    let program = AuthoredProgram {
        root: &directory,
        entry: ENTRY,
        sha256: SOURCE_SHA256,
        contract: original.contract().unwrap(),
    };
    let mut identities = BTreeSet::new();
    for case in CASES {
        let edits = case.edits();
        let amended = Workload::authored(program, &edits, WorkloadLimits::default()).unwrap();
        let record = serde_json::to_value(&amended).unwrap();
        let recorded = record["sources"][0]["edits"].as_array().unwrap();
        assert_eq!(recorded.len(), edits.len());
        for (recorded, edit) in recorded.iter().zip(&edits) {
            assert_eq!(recorded["before"], edit.expected.to_string());
            assert_eq!(recorded["after"], edit.replacement.to_string());
        }
        assert_eq!(amended.is_amended(), case != DEFAULT);
        assert_eq!(amended.contract().is_none(), amended.is_amended());
        assert!(identities.insert(amended.identity().to_owned()));
    }
    let stale = ConstantAmendment {
        source_path: ENTRY,
        name: "terms",
        expected: 67,
        replacement: 52,
    };
    assert!(Workload::authored(program, &[stale], WorkloadLimits::default()).is_err());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "requires clingo: every 2QBF case has the formula's complete family"]
fn two_qbf_complete_families_satisfy_the_formula() {
    use std::time::Duration;
    use zetesis_clingo_support as oracle;
    for case in CASES {
        let constants = case
            .edits()
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
            .chain([ENTRY.to_owned()])
            .collect::<Vec<_>>();
        let run = oracle::run_in(
            &root().join("scalability/two-qbf"),
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
