//! Maintained authored fixtures and a finite worker-scaling population.
//!
//! Parametric fixtures use the reviewed scalability manifest; fixed fixtures
//! carry their reviewed source digests and typed display contracts here. Workload
//! identities retain the actual contract and original/derived source hashes.
//! This is authored evidence, not upstream corpus-cleaning provenance or a
//! generated-family proof.

use std::{
    num::{NonZeroU64, NonZeroUsize},
    path::Path,
};

use serde::Deserialize;

use super::{
    Error,
    matrix::{self, AuthoredProgram, ConstantAmendment, Workload, WorkloadLimits},
};
use crate::{
    examples,
    selected::{FormulaJoins, NativeExecution, SearchMethod},
};

/// Reviewed metadata for the two default scalability fixtures.
pub const MANIFEST_SHA256: &str =
    "eabb3ca0161beca0816f7438439818aa4920465b979c9618c764f3c3b3bb1870";
const SUDOKU_SHA256: &str = "9ee5cb65a0ad7e563af9378a46e04b3a19762850ce7cb476ad7d9adbe27466ef";
const EINSTEIN_SHA256: &str = "d142a0b2f515954e6d4bbceeebac7f25f87f040ebc72897049c666dd9db1652b";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    description: String,
    reference_toolchain: String,
    cases: Vec<Fixture>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    path: String,
    scaling_constant: String,
    default_value: i32,
    contract: examples::Contract,
    source_sha256: String,
}

fn fixtures(root: &Path, limits: WorkloadLimits) -> Result<Vec<Fixture>, Error> {
    let root = examples::files::canonical(root).map_err(Error::Corpus)?;
    let path =
        examples::files::confined(&root, "scalability/manifest.json").map_err(Error::Corpus)?;
    let bytes = examples::files::read(
        &path,
        limits.metadata_bytes,
        examples::Resource::ManifestBytes,
    )
    .map_err(Error::Corpus)?;
    examples::files::digest("scalability/manifest.json", &bytes, MANIFEST_SHA256)
        .map_err(Error::Corpus)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|error| Error::Corpus(examples::Error::Json(error)))?;
    if manifest.schema_version != 1
        || manifest.description.is_empty()
        || manifest.reference_toolchain != "clingo version 5.8.2"
        || manifest.cases.len() != 2
    {
        return Err(Error::Configuration("unsupported scalability manifest"));
    }
    Ok(manifest.cases)
}

fn workload(
    root: &Path,
    fixture: &Fixture,
    size: i32,
    limits: WorkloadLimits,
) -> Result<Workload, Error> {
    let entry = format!("scalability/{}", fixture.path);
    let amendment = ConstantAmendment {
        source_path: &entry,
        name: &fixture.scaling_constant,
        expected: fixture.default_value,
        replacement: size,
    };
    Workload::authored(
        AuthoredProgram {
            root,
            entry: &entry,
            sha256: &fixture.source_sha256,
            contract: &fixture.contract,
        },
        if size == fixture.default_value {
            &[]
        } else {
            std::slice::from_ref(&amendment)
        },
        limits,
    )
}

/// Admit queens at n=8 and pigeonhole at h=7, with their original contracts.
/// `root` is the repository's `examples` directory.
///
/// # Errors
/// Refuses changed metadata/sources, unsupported syntax or admission limits.
pub fn defaults(root: &Path, limits: WorkloadLimits) -> Result<Vec<Workload>, Error> {
    fixtures(root, limits)?
        .iter()
        .map(|fixture| workload(root, fixture, fixture.default_value, limits))
        .collect()
}

/// Admit the authored Sudoku fixture with its unique complete grid display.
/// Eight given digits per row make this a grounding and language workload,
/// not a difficult Sudoku search instance. `root` is the examples directory.
///
/// # Errors
/// Refuses changed source bytes, unsupported syntax or admission limits.
pub fn sudoku(root: &Path, limits: WorkloadLimits) -> Result<Workload, Error> {
    let grid = (1..=9)
        .flat_map(|row| {
            (1..=9).map(move |column| {
                let digit = ((row - 1) * 3 + (row - 1) / 3 + column - 1) % 9 + 1;
                format!("digit({row},{column},{digit})")
            })
        })
        .collect();
    let contract = examples::Contract::complete_family(NonZeroU64::MIN)
        .with_witnesses(vec![grid])
        .map_err(Error::Corpus)?;
    Workload::authored(
        AuthoredProgram {
            root,
            entry: "sudoku.lp",
            sha256: SUDOKU_SHA256,
            contract: &contract,
        },
        &[],
        limits,
    )
}

/// Admit the unchanged Einstein riddle with its unique five-house display.
///
/// # Errors
/// Refuses changed source bytes, unsupported syntax or admission limits.
pub fn einstein(root: &Path, limits: WorkloadLimits) -> Result<Workload, Error> {
    let witness = [
        r#"solution(1,"Yellow","The Norwegian","Fox","Water","Kools")"#,
        r#"solution(2,"Blue","The Ukrainian","Horse","Tea","Chesterfields")"#,
        r#"solution(3,"Red","The Englishman","Snail","Milk","Old Gold")"#,
        r#"solution(4,"Ivory","The Spaniard","Dog","Orange Juice","Lucky Strikes")"#,
        r#"solution(5,"Green","The Japanese","Zebra","Coffee","Parliaments")"#,
    ]
    .map(str::to_owned)
    .to_vec();
    let contract = examples::Contract::complete_family(NonZeroU64::MIN)
        .with_witnesses(vec![witness])
        .map_err(Error::Corpus)?;
    Workload::authored(
        AuthoredProgram {
            root,
            entry: "einstein-riddle.lp",
            sha256: EINSTEIN_SHA256,
            contract: &contract,
        },
        &[],
        limits,
    )
}

/// Queens at n=8/9/10 and pigeonhole at h=5/6/7, followed by unchanged queens
/// variant 02, SEND+MORE=MONEY, task allocation and the authored Sudoku grid.
/// Einstein is an optional eleventh case. Amended sizes are qualified against
/// a complete reference enumeration.
///
/// # Errors
/// Refuses fixture/corpus identity changes and checked-amendment limits.
pub fn workloads(
    corpus: &examples::Corpus,
    root: &Path,
    include_einstein: bool,
    limits: WorkloadLimits,
) -> Result<Vec<Workload>, Error> {
    let fixtures = fixtures(root, limits)?;
    let mut result = Vec::with_capacity(11);
    for (fixture, sizes) in fixtures.iter().zip([[8, 9, 10], [5, 6, 7]]) {
        for size in sizes {
            result.push(workload(root, fixture, size, limits)?);
        }
    }
    for case in [
        super::Case::Queens02,
        super::Case::Send,
        super::Case::TaskAllocation,
    ] {
        result.push(Workload::original(corpus, case.path(), limits)?);
    }
    result.push(sudoku(root, limits)?);
    if include_einstein {
        result.push(einstein(root, limits)?);
    }
    Ok(result)
}

/// CPU eager/indexed region search at 1, 2, 4, 8 and 14 workers, with one
/// completion worker. An optional expansion ceiling is retained in every profile.
#[must_use]
pub fn profiles(max_expansion_work: Option<usize>) -> Vec<NativeExecution> {
    const WORKERS: [NonZeroUsize; 5] = [
        NonZeroUsize::MIN,
        NonZeroUsize::new(2).unwrap(),
        NonZeroUsize::new(4).unwrap(),
        NonZeroUsize::new(8).unwrap(),
        NonZeroUsize::new(14).unwrap(),
    ];
    WORKERS
        .map(|workers| NativeExecution {
            workers,
            formula_joins: Some(FormulaJoins::Indexed),
            search: Some(SearchMethod::Regions),
            max_expansion_work,
            ..NativeExecution::default()
        })
        .to_vec()
}

/// Run the maintained scalability population through the shared matrix owner.
/// The request must select [`matrix::Suite::Scalability`]. Qualification-only
/// and measurement plans use the same workloads, provenance and complete-family
/// checks. All request limits are honored without raising them. The caller owns
/// cancellation and publication of the returned report.
///
/// # Errors
/// Refuses a different suite, changed input identities, workload admission
/// limits, or the matrix's pre-launch configuration and source failures.
pub fn run_with_cancellation(
    request: &matrix::Request<'_>,
    root: &Path,
    include_einstein: bool,
    invocation: matrix::NativeInvocation,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<matrix::Report, Error> {
    if request.plan.suite() != matrix::Suite::Scalability {
        return Err(Error::Configuration(
            "authored scalability requires the scalability suite",
        ));
    }
    let corpus = examples::load(request.corpus, request.limits.corpus).map_err(Error::Corpus)?;
    let limits = WorkloadLimits {
        source_bytes: request.limits.corpus.source_bytes,
        total_source_bytes: request.limits.corpus.total_source_bytes,
        metadata_bytes: request.limits.corpus.manifest_bytes,
        ..WorkloadLimits::default()
    };
    let workloads = workloads(&corpus, root, include_einstein, limits)?;
    matrix::run_workloads_with_cancellation(request, &workloads, invocation, cancelled)
}
