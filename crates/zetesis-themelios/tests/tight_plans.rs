//! Source-to-certificate boundaries; classification reads the completed DAG.

use std::path::{Path, PathBuf};

use zetesis_cpu::Control;
use zetesis_ferraris::{
    Interpretation, TightCheckLimits, TightError, TightPlan, TightPlanLimits, TightVerdict,
};
use zetesis_themelios::{
    AdmissionOptions, BundleAdmissionOptions, BundleLimits, ExpansionLimits, FormulaLimits,
    SourceBundle, admit_bundle_formula, admit_formula,
};

#[test]
fn source_normal_choice_and_frozen_aggregate_guards_are_certified() {
    for (source, models) in [
        ("a.", 1),
        ("{a}.", 2),
        ("{a}. b :- a.", 2),
        ("1 { a; b } 1.", 2),
        ("{a;b}. :- #count{1:a;2:b} > 1.", 3),
        ("{a;b}. c :- #count{1:a;2:b} >= 1.", 4),
        ("{a;b}. c :- not #count{1:a;2:b} = 1.", 4),
        ("a :- not not a.", 2),
    ] {
        let admitted = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let theory = admitted.theory();
        let plan = TightPlan::compile(theory, TightPlanLimits::default(), &Control::default())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        let mut accepted = 0;
        for mask in 0..1usize << theory.atom_count() {
            let candidate = Interpretation::new(
                theory,
                (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
            )
            .unwrap();
            let checked = plan
                .check(&candidate, TightCheckLimits::default(), &Control::default())
                .unwrap();
            let reference = zetesis_ferraris::check(
                theory,
                &candidate,
                zetesis_ferraris::Limits::default(),
                &Control::default(),
            )
            .unwrap();
            assert_eq!(
                checked.verdict == TightVerdict::Stable,
                reference.accepted(),
                "{source}"
            );
            accepted += usize::from(checked.verdict == TightVerdict::Stable);
        }
        assert_eq!(accepted, models, "{source}");
    }
}

#[test]
fn source_class_labels_cannot_hide_positive_aggregate_or_conditional_dependencies() {
    for source in [
        "{p}. p :- #count{1:p} > 0.",
        "p :- p:p.",
        "a|b. a:-b. b:-a.",
    ] {
        let admitted = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let result = TightPlan::compile(
            admitted.theory(),
            TightPlanLimits::default(),
            &Control::default(),
        );
        assert!(
            matches!(
                result,
                Err(TightError::PositiveCycle { .. }
                    | TightError::UnsupportedBody { .. }
                    | TightError::UnsupportedRoot { .. })
            ),
            "{source}: {result:?}"
        );
    }
}

fn files(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, output);
        } else if path.extension().is_some_and(|ext| ext == "lp") {
            output.push(path);
        }
    }
}

#[test]
#[ignore = "records completed-theory eligibility over the 94 original corpus inputs"]
fn original_corpus_eligibility() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation/corpus/kr-domains");
    let mut paths = Vec::new();
    files(&root.join("scenarios"), &mut paths);
    files(&root.join("standalone"), &mut paths);
    paths.sort();
    assert_eq!(paths.len(), 94);
    for path in paths {
        let bundle = SourceBundle::load(&path, BundleLimits::default()).unwrap();
        let admitted = admit_bundle_formula(
            bundle,
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let theory = admitted.theory();
        let mut record = serde_json::json!({
            "source": path.strip_prefix(&root).unwrap().to_str().unwrap(),
            "atoms": theory.atom_count(), "nodes": theory.nodes().len(), "roots": theory.roots().len(),
        });
        match TightPlan::compile(theory, TightPlanLimits::default(), &Control::default()) {
            Ok(plan) => {
                let stats = plan.statistics();
                record["certified"] = true.into();
                record["producers"] = plan.producers().len().into();
                record["dependencies"] = stats.dependencies.into();
                record["construction_bytes"] = stats.construction_bytes.into();
                record["resident_bytes"] = stats.resident_bytes.into();
                record["work"] = stats.work.into();
            }
            Err(error) => {
                assert!(matches!(
                    error,
                    TightError::PositiveCycle { .. }
                        | TightError::UnsupportedBody { .. }
                        | TightError::UnsupportedRoot { .. }
                ));
                record["certified"] = false.into();
                record["fallback"] = format!("{error:?}").into();
            }
        }
        println!("{record}");
    }
}
