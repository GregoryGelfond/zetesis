//! Compare one constant-derived source through the bounded validation library.

// ANCHOR: example
use std::{error::Error, num::NonZeroUsize, path::PathBuf};
use zetesis_validation::{
    answers::native_json,
    examples,
    performance::{self, matrix},
    selected::{Backend, Grounder, NativeExecution},
};

const ENTRY: &str = "standalone/n-queens/variant-01.lp";

fn workload(corpus: &examples::Corpus) -> Result<matrix::Workload, performance::Error> {
    matrix::Workload::amended(
        corpus,
        ENTRY,
        &[matrix::ConstantAmendment {
            source_path: ENTRY,
            name: "n",
            expected: 8,
            replacement: 4,
        }],
        matrix::WorkloadLimits::default(),
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    let paths: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [corpus_path, native, reference, destination] = paths.as_slice() else {
        return Err(
            "usage: book-workload CORPUS ABSOLUTE_ZETESIS ABSOLUTE_CLINGO NEW_REPORT".into(),
        );
    };
    let limits = performance::Limits::default();
    let corpus = examples::load(corpus_path, limits.corpus)?;
    let selected = workload(&corpus)?;
    let profile = NativeExecution {
        backend: Backend::Cpu,
        grounder: Grounder::Eager,
        threads: NonZeroUsize::MIN,
        ..NativeExecution::default()
    };
    let request = matrix::Request {
        tool: matrix::Tool {
            name: "book-workload".into(),
            // Cargo supplies the package version; a build outside Cargo has none.
            version: option_env!("CARGO_PKG_VERSION")
                .unwrap_or("unversioned")
                .into(),
        },
        corpus: corpus_path,
        native,
        // An amended board has no recorded contract: clingo establishes its family.
        reference: Some(matrix::Reference {
            executable: reference,
            policy: matrix::ReferencePolicy::AllPhases,
        }),
        report: destination,
        plan: matrix::Plan::new(
            matrix::Suite::Queens,
            vec![profile],
            NonZeroUsize::MIN,
            0,
            1,
        )?,
        limits,
        native_answers: native_json::Limits::default(),
        max_spelling_bytes: limits.process.max_output_bytes,
        helper: None,
    };
    let report = matrix::run_workloads(&request, &[selected])?;
    report.publish()?;
    println!("Saved {}", destination.display());
    if !report.passed() {
        return Err("comparison did not fully pass; inspect the saved report".into());
    }
    Ok(())
}
// ANCHOR_END: example

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::path::Path;

    #[test]
    fn prepared_workload_identifies_the_n4_source() {
        let corpus = examples::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/correctness"),
            examples::Limits::default(),
        )
        .unwrap();
        let source = corpus
            .files()
            .iter()
            .find(|source| source.path() == ENTRY)
            .unwrap();
        assert_eq!(source.source().matches("#const n = 8.").count(), 1);
        let expected = source.source().replace("#const n = 8.", "#const n = 4.");
        let prepared = workload(&corpus).unwrap();
        let encoded = serde_json::to_value(&prepared).unwrap();
        let sources = encoded["sources"].as_array().unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["path"], ENTRY);
        assert_eq!(sources[0]["base_sha256"], source.source_sha256());
        assert_eq!(
            sources[0]["derived_sha256"],
            format!("{:x}", Sha256::digest(expected.as_bytes()))
        );
        assert_eq!(sources[0]["derived_bytes"], expected.len());
        let original =
            matrix::Workload::original(&corpus, ENTRY, matrix::WorkloadLimits::default()).unwrap();
        assert_ne!(prepared.identity(), original.identity());
    }
}
