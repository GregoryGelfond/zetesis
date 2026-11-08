//! Strategic companies, a disjunctive program whose answer sets are the minimal
//! covering, control-closed sets, checked against a direct enumeration of them.
use super::{BTreeSet, Workload, WorkloadLimits, bundles, examples, root};
use zetesis_validation::performance::matrix::{AuthoredProgram, ConstantAmendment};

const ENTRY: &str = "strategic_companies.lp";
const SOURCE_SHA256: &str = "cf562da040add78158e15760e7cdd2c2fa859bb9dd7e2c4f1194494a6cf8cc93";
const MANIFEST_SHA256: &str = "546e90661ee87f91956acea79a7e252dc36dc119bfba034daf7c4d77c5148425";

/// One instance's constants, as the program names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Instance {
    companies: i64,
    products: i64,
    seed: i64,
}

const DEFAULT: Instance = Instance {
    companies: 12,
    products: 24,
    seed: 1,
};

/// The default, another seed at the default size, and two smaller holdings.
const CASES: [Instance; 4] = [
    DEFAULT,
    Instance { seed: 2, ..DEFAULT },
    Instance {
        companies: 8,
        products: 16,
        seed: 2,
    },
    Instance {
        companies: 10,
        products: 20,
        seed: 3,
    },
];

impl Instance {
    /// The program's hash, evaluated as the program does; every intermediate
    /// value stays below 2^31.
    fn draw(self, kind: i64, index: i64, slot: i64) -> i64 {
        let square = |value: i64, offset: i64| (value * value + offset) % 46_337;
        let start =
            (self.seed * 7919 + index * 104_729 + slot * 1_299_709 + kind * 15_485_863) % 46_337;
        square(square(square(start, 12_345), 54_321), 7)
    }

    /// Each product's producers, and each controlled company's controllers, as
    /// bit masks over companies `1..=companies`.
    fn holding(self) -> (Vec<u32>, Vec<(u32, u32)>) {
        let bit = |company: i64| 1_u32 << (company - 1);
        let producers = (1..=self.products)
            .map(|product| {
                (1..=4)
                    .map(|slot| bit(self.draw(0, product, slot) % self.companies + 1))
                    .fold(0, |mask, company| mask | company)
            })
            .collect();
        let control = (1..=self.companies)
            .filter(|&company| self.draw(1, company, 0) % 3 == 0)
            .map(|company| {
                let controllers = (1..=4)
                    .map(|slot| {
                        let offset = self.draw(1, company, slot) % (self.companies - 1) + 1;
                        bit((company - 1 + offset) % self.companies + 1)
                    })
                    .fold(0, |mask, controller| mask | controller);
                (bit(company), controllers)
            })
            .collect();
        (producers, control)
    }

    /// The complete answer family: the inclusion-minimal sets that meet every
    /// product's producers and hold every company whose controllers they hold.
    fn family(self) -> BTreeSet<Vec<String>> {
        let (producers, control) = self.holding();
        let qualifies = |set: u32| {
            producers.iter().all(|&mask| set & mask != 0)
                && control.iter().all(|&(company, controllers)| {
                    set & controllers != controllers || set & company != 0
                })
        };
        let qualifying = (0..1_u32 << self.companies)
            .filter(|&set| qualifies(set))
            .collect::<Vec<_>>();
        qualifying
            .iter()
            .filter(|&&set| {
                !qualifying
                    .iter()
                    .any(|&other| other != set && other & !set == 0)
            })
            .map(|&set| {
                let mut display = (1..=self.companies)
                    .filter(|company| set & (1 << (company - 1)) != 0)
                    .map(|company| format!("strat({company})"))
                    .collect::<Vec<_>>();
                display.sort();
                display
            })
            .collect()
    }

    /// The complete display contract the family determines.
    fn expected(self) -> examples::Contract {
        let family = self.family();
        serde_json::from_value(serde_json::json!({
            "satisfiability": "sat", "family": "all",
            "model_count": family.len(), "cost": null,
            "witnesses": family, "required_symbols": [], "notes": []
        }))
        .unwrap()
    }

    /// The `#const` edits that turn the default into this instance.
    fn edits(self) -> Vec<ConstantAmendment<'static>> {
        [
            ("companies", DEFAULT.companies, self.companies),
            ("products", DEFAULT.products, self.products),
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
        &root().join("scalability/strategic-companies/manifest.json"),
        WorkloadLimits::default(),
    )
    .unwrap();
    assert_eq!(workloads.len(), 1);
    workloads.pop().unwrap()
}

#[test]
fn strategic_companies_seal_their_source_and_manifest() {
    let workload = workload();
    assert!(workload.is_bundle());
    assert_eq!(workload.entry(), ENTRY);
    let record = serde_json::to_value(&workload).unwrap();
    assert_eq!(record["bundle"]["manifest"]["sha256"], MANIFEST_SHA256);
    assert_eq!(record["bundle"]["files"][0]["sha256"], SOURCE_SHA256);
}

#[test]
fn strategic_companies_default_contract_lists_every_strategic_set() {
    let contract = workload().contract().unwrap().clone();
    let expected = DEFAULT.expected();
    assert_eq!(contract.satisfiability(), examples::Satisfiability::Sat);
    assert_eq!(contract.family(), examples::Family::All);
    assert_eq!(contract.model_count(), expected.model_count());
    assert_eq!(contract.cost(), None);
    assert_eq!(
        contract.witnesses().iter().collect::<BTreeSet<_>>(),
        expected.witnesses().iter().collect::<BTreeSet<_>>()
    );
}

#[test]
fn strategic_companies_default_holding_has_controlled_companies() {
    // Without a control rule the minimality would be a covering's alone.
    let (_, control) = DEFAULT.holding();
    assert!(!control.is_empty());
}

#[test]
fn strategic_companies_constant_edits_retain_checked_provenance() {
    let original = workload();
    let directory = root().join("scalability/strategic-companies");
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
        name: "companies",
        expected: 13,
        replacement: 8,
    };
    assert!(Workload::authored(program, &[stale], WorkloadLimits::default()).is_err());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "requires clingo: every strategic-companies case lists every strategic set"]
fn strategic_companies_complete_families_list_every_strategic_set() {
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
            &root().join("scalability/strategic-companies"),
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
