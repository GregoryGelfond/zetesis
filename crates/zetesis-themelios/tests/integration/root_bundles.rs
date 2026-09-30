//! Ordered roots share original identities, source budgets and global constants.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use themelios_base::source::SourceId;
use zetesis_test_support::harness;
use zetesis_themelios::{
    BundleAdmissionError, BundleAdmissionOptions, BundleError, BundleLimits, BundleResource,
    ExpansionLimits, IncludeResolution, SourceBundle, admit_bundle_extended,
};

struct Fixture(tempfile::TempDir);
impl Fixture {
    fn new() -> Self {
        Self(tempfile::tempdir().expect("fixture directory"))
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, source).unwrap();
        path
    }
    fn load(&self, roots: &[&str]) -> SourceBundle {
        SourceBundle::load_many(
            roots.iter().map(|root| self.0.path().join(root)),
            BundleLimits::default(),
        )
        .unwrap()
    }
    /// Remove the directory, failing the test if it cannot be removed.
    fn close(self) {
        self.0
            .close()
            .expect("only this fixture's original sources");
    }
}

#[test]
fn ordered_occurrences_and_shared_includes_keep_unique_original_sources() {
    let fixture = Fixture::new();
    fixture.write("first.lp", "#include \"shared.lp\". p(n).");
    fixture.write("second.lp", "#include \"shared.lp\". q(n).");
    fixture.write("shared.lp", "#const n=2.");
    let bundle = fixture.load(&["first.lp", "second.lp", "shared.lp", "first.lp"]);
    let roots: Vec<_> = bundle
        .roots()
        .iter()
        .map(zetesis_themelios::BundleRoot::id)
        .collect();
    assert_eq!(roots, [0, 2, 1, 0].map(SourceId::new));
    assert_eq!(bundle.entry(), SourceId::new(0));
    assert_eq!(bundle.sources().len(), 3);
    assert_eq!(
        bundle.roots()[1].requested_path(),
        fixture.0.path().join("second.lp")
    );
    let expected_bytes: usize = bundle
        .sources()
        .iter()
        .map(|source| source.source().text().len())
        .sum();
    assert_eq!(bundle.total_bytes(), expected_bytes);
    let admitted = admit_bundle_extended(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .expect("exact root and include spellings are included once");
    assert_eq!(admitted.program().templates().len(), 2);
    let owners: Vec<_> = admitted
        .template_origins()
        .iter()
        .flatten()
        .map(|location| location.source)
        .collect();
    assert!(owners.contains(&SourceId::new(0)));
    assert!(owners.contains(&SourceId::new(2)));
    fixture.close();
}

#[test]
fn later_roots_supply_global_constants_without_flattening_evidence() {
    let fixture = Fixture::new();
    fixture.write("rules.lp", "% original rule\np(lower..upper). #show p/1.");
    fixture.write("constants.lp", "#const upper=lower+1. #const lower=1.");
    let admitted = admit_bundle_extended(
        fixture.load(&["rules.lp", "constants.lp"]),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    assert_eq!(admitted.program().templates().len(), 2);
    assert!(
        admitted
            .template_origins()
            .iter()
            .flatten()
            .all(|origin| origin.source == SourceId::new(0))
    );
    assert_eq!(
        admitted
            .bundle()
            .get(SourceId::new(1))
            .unwrap()
            .source()
            .text(),
        "#const upper=lower+1. #const lower=1."
    );
    fixture.close();
}

#[test]
fn root_occurrences_have_an_independent_inclusive_bound() {
    assert!(matches!(
        SourceBundle::load_many(Vec::<PathBuf>::new(), BundleLimits::default()),
        Err(BundleError::EmptyRoots)
    ));
    let fixture = Fixture::new();
    let path = fixture.write("root.lp", "p.");
    for maximum in [0, 1, 2] {
        let loaded = SourceBundle::load_many(
            [&path, &path],
            BundleLimits {
                max_roots: maximum,
                ..BundleLimits::default()
            },
        );
        if maximum == 2 {
            let bundle = loaded.unwrap();
            assert_eq!(bundle.roots().len(), 2);
            assert_eq!(bundle.sources().len(), 1);
            assert_eq!(bundle.total_bytes(), 2);
        } else {
            assert!(matches!(loaded, Err(BundleError::Limit {
                resource: BundleResource::Roots, observed, limit, ..
            }) if limit == maximum as u128 && observed == limit + 1));
        }
    }
    fixture.close();
}

#[test]
fn file_and_byte_allowances_apply_across_all_roots_before_parsing() {
    let fixture = Fixture::new();
    let first = fixture.write("first.lp", "p.");
    let second = fixture.write("second.lp", "broken");
    for (limits, resource) in [
        (
            BundleLimits {
                max_files: 1,
                ..BundleLimits::default()
            },
            BundleResource::Files,
        ),
        (
            BundleLimits {
                max_total_bytes: 7,
                ..BundleLimits::default()
            },
            BundleResource::TotalBytes,
        ),
    ] {
        assert!(
            matches!(SourceBundle::load_many([&first, &second], limits), Err(BundleError::Limit {
            resource: actual, including: None, path, ..
        }) if actual == resource && path.ends_with("second.lp"))
        );
    }
    fixture.close();
}

#[test]
fn later_root_parse_and_semantic_refusals_keep_original_source_ids() {
    let fixture = Fixture::new();
    fixture.write("first.lp", "p.");
    fixture.write("second.lp", "broken(.");
    let error = SourceBundle::load_many(
        [
            fixture.0.path().join("first.lp"),
            fixture.0.path().join("second.lp"),
        ],
        BundleLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(error, BundleError::Syntax { source, .. }
        if source.id() == SourceId::new(1) && source.text() == "broken(."));
    fixture.write("second.lp", "#program other. q.");
    let error = admit_bundle_extended(
        fixture.load(&["first.lp", "second.lp"]),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.bundle().roots().len(), 2);
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.primary().location.source == SourceId::new(1))
    );
    fixture.close();
}

#[test]
fn root_aliases_do_not_silently_erase_repeated_definitions() {
    let fixture = Fixture::new();
    fixture.write("data.lp", "#const n=1. p(n).");
    let error = admit_bundle_extended(
        fixture.load(&["data.lp", "./data.lp"]),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error.error(),
        BundleAdmissionError::RootAlias { .. }
    ));
    fixture.write("first.lp", "#include \"data.lp\".");
    let error = admit_bundle_extended(
        fixture.load(&["first.lp", "./data.lp"]),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error.error(),
        BundleAdmissionError::RootAlias { .. }
    ));
    fixture.close();
}

#[test]
fn cwd_resolution_is_captured_before_loading_and_retained_for_admission() {
    const CHILD: &str = "ZETESIS_BUNDLE_CWD_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let bundle =
            SourceBundle::load_many(["sub/entry.lp", "second.lp"], BundleLimits::default())
                .unwrap();
        let entry = bundle.get(bundle.entry()).unwrap();
        assert_eq!(
            entry.includes()[0].resolution(),
            IncludeResolution::WorkingDirectory
        );
        assert_eq!(entry.includes()[0].resolved_path(), Path::new("data.lp"));
        assert_eq!(
            entry.includes()[1].resolution(),
            IncludeResolution::IncludingDirectory
        );
        assert_eq!(
            entry.includes()[1].resolved_path(),
            Path::new("sub/fallback.lp")
        );
        assert!(
            bundle
                .sources()
                .iter()
                .any(|source| source.source().text() == "cwd.")
        );
        assert!(
            !bundle
                .sources()
                .iter()
                .any(|source| source.source().text() == "shadowed.")
        );
        // This helper runs alone in a separate process; no parallel test sees
        // its cwd change. Admission must use captured resolution evidence.
        std::env::set_current_dir("sub").unwrap();
        let admitted = admit_bundle_extended(
            bundle,
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
        )
        .unwrap();
        assert_eq!(admitted.program().templates().len(), 4);
        return;
    }
    let fixture = Fixture::new();
    fixture.write(
        "sub/entry.lp",
        "#include \"data.lp\". #include \"fallback.lp\". #include \"block/data.lp\".",
    );
    fixture.write("data.lp", "cwd.");
    fixture.write("sub/data.lp", "shadowed.");
    fixture.write("sub/fallback.lp", "fallback.");
    fixture.write("second.lp", "second.");
    fixture.write("block", "a regular file, not a directory");
    fixture.write("sub/block/data.lp", "not_directory_fallback.");
    let name = harness::test_name(
        module_path!(),
        "cwd_resolution_is_captured_before_loading_and_retained_for_admission",
    );
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name.as_str(), "--test-threads=1"])
        .env(CHILD, "1")
        .current_dir(fixture.0.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let output = String::from_utf8_lossy(&result.stdout);
    assert!(result.status.success(), "{output}");
    // A name the harness does not know runs nothing and still succeeds.
    assert!(output.contains("test result: ok. 1 passed"), "{output}");
    fixture.close();
}
