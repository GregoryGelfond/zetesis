//! Every comparison against clingo in the maintained sources is run by the
//! oracle gate. Such a test is ignored, since the oracle is external, and
//! `scripts/check.sh oracle` runs the ignored tests of the test targets it
//! names by hand; a target left out of that list is run by nothing.

#[path = "support/authored_sources.rs"]
mod authored_sources;
#[path = "support/clingo_comparisons.rs"]
mod clingo_comparisons;
#[path = "support/oracle_campaigns.rs"]
mod oracle_campaigns;

use std::fs;
use std::path::{Path, PathBuf};

use clingo_comparisons::{Comparison, comparisons};
use oracle_campaigns::{Campaign, campaigns};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn oracle_gate(root: &Path) -> Vec<Campaign> {
    let script = fs::read_to_string(root.join("scripts/check.sh")).unwrap();
    campaigns(&script).unwrap()
}

fn runs(campaigns: &[Campaign], comparison: &Comparison) -> bool {
    comparison.target.as_deref().is_some_and(|target| {
        campaigns
            .iter()
            .any(|campaign| campaign.runs(&comparison.package, target, &comparison.name))
    })
}

#[test]
fn the_oracle_gate_runs_every_clingo_comparison() {
    let root = repository();
    let gate = oracle_gate(&root);
    let sources = authored_sources::inventory(&root).unwrap();
    let found = comparisons(&root, &sources).unwrap();
    assert!(
        !found.is_empty(),
        "the maintained sources hold clingo comparisons"
    );
    let unrun: Vec<String> = found
        .iter()
        .filter(|comparison| !runs(&gate, comparison))
        .map(ToString::to_string)
        .collect();
    assert!(
        unrun.is_empty(),
        "clingo comparisons no oracle campaign runs:\n{}",
        unrun.join("\n")
    );
    println!(
        "clingo_comparisons={} oracle_campaigns={}",
        found.len(),
        gate.len()
    );
}

#[test]
fn every_target_the_oracle_gate_names_holds_a_clingo_comparison() {
    let root = repository();
    let gate = oracle_gate(&root);
    let sources = authored_sources::inventory(&root).unwrap();
    let found = comparisons(&root, &sources).unwrap();
    let idle: Vec<String> = gate
        .iter()
        .flat_map(|campaign| {
            campaign
                .targets
                .iter()
                .map(move |target| (campaign, target))
        })
        .filter(|(campaign, target)| {
            !found.iter().any(|comparison| {
                comparison.package == campaign.package
                    && comparison.target.as_deref() == Some(target.as_str())
            })
        })
        .map(|(campaign, target)| {
            format!(
                "scripts/check.sh:{}: -p {} --test {}",
                campaign.line, campaign.package, target
            )
        })
        .collect();
    assert!(
        idle.is_empty(),
        "oracle campaign targets holding no clingo comparison:\n{}",
        idle.join("\n")
    );
}

const SCRIPT: &str = "oracle_test() {\n    cargo test \"$@\"\n}\n\
    oracle_test --locked --no-fail-fast -p example --test alpha --test beta -- --ignored --nocapture\n";

#[test]
fn a_campaign_names_its_package_and_its_targets() {
    let found = campaigns(SCRIPT).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 4);
    assert_eq!(found[0].package, "example");
    assert_eq!(found[0].targets, ["alpha", "beta"]);
    assert_eq!(found[0].filter, None);
    assert!(found[0].ignored);
}

#[test]
fn a_campaign_runs_the_targets_it_names_in_its_package() {
    let found = campaigns(SCRIPT).unwrap();
    assert!(found[0].runs("example", "alpha", "any_test"));
    assert!(found[0].runs("example", "beta", "any_test"));
    assert!(!found[0].runs("example", "gamma", "any_test"));
    assert!(!found[0].runs("other", "alpha", "any_test"));
}

#[test]
fn a_campaign_filter_selects_the_tests_it_names() {
    let script =
        "oracle_test --locked -p example --test alpha original_sources_agree -- --ignored\n";
    let found = campaigns(script).unwrap();
    assert_eq!(found[0].filter.as_deref(), Some("original_sources_agree"));
    assert!(found[0].runs("example", "alpha", "original_sources_agree"));
    assert!(found[0].runs("example", "alpha", "the_original_sources_agree_with_clingo"));
    assert!(!found[0].runs("example", "alpha", "another_comparison"));
}

#[test]
fn a_campaign_without_ignored_runs_no_comparison() {
    let script = "oracle_test --locked -p example --test alpha -- --nocapture\n";
    let found = campaigns(script).unwrap();
    assert!(!found[0].ignored);
    assert!(!found[0].runs("example", "alpha", "any_test"));
}

#[test]
fn a_campaign_needs_a_package_and_test_targets() {
    assert!(campaigns("oracle_test --locked --test alpha -- --ignored\n").is_err());
    assert!(campaigns("oracle_test --locked -p example -- --ignored\n").is_err());
    assert!(campaigns("oracle_test --locked -p example --lib -- --ignored\n").is_err());
}

fn write(root: &Path, path: &str, text: &str) -> PathBuf {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

fn package(root: &Path, directory: &str, name: &str) {
    write(
        root,
        &format!("{directory}/Cargo.toml"),
        &format!("[package]\nname = \"{name}\"\nversion = \"0.0.0\"\n"),
    );
}

fn summary(found: &[Comparison]) -> Vec<(String, Option<String>, String)> {
    found
        .iter()
        .map(|comparison| {
            (
                comparison.package.clone(),
                comparison.target.clone(),
                comparison.name.clone(),
            )
        })
        .collect()
}

#[test]
fn a_reason_names_the_oracle_in_any_spelling() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let alpha = write(
        root,
        "crates/example/tests/alpha.rs",
        r#"#[test]
#[ignore = "requires an independently installed clingo"]
fn lower_case() {}
#[test]
#[ignore = "requires absolute CLINGO; complete references"]
fn upper_case() {}
#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn another_oracle() {}
#[test]
fn portable() {}
"#,
    );
    let found = comparisons(root, &[alpha]).unwrap();
    assert_eq!(
        summary(&found),
        [
            ("example".into(), Some("alpha".into()), "lower_case".into()),
            ("example".into(), Some("alpha".into()), "upper_case".into()),
        ]
    );
}

#[test]
fn a_comparison_belongs_to_each_test_that_includes_its_module() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let include = "#[path = \"support/shared.rs\"]\nmod shared;\n";
    let alpha = write(root, "crates/example/tests/alpha.rs", include);
    let beta = write(root, "crates/example/tests/beta.rs", include);
    let shared = write(
        root,
        "crates/example/tests/support/shared.rs",
        "#[test]\n#[ignore = \"requires independent clingo\"]\nfn shared_comparison() {}\n",
    );
    let found = comparisons(root, &[alpha, beta, shared]).unwrap();
    assert_eq!(
        summary(&found),
        [
            (
                "example".into(),
                Some("alpha".into()),
                "shared_comparison".into()
            ),
            (
                "example".into(),
                Some("beta".into()),
                "shared_comparison".into()
            ),
        ]
    );
}

#[test]
fn a_module_declared_without_a_path_is_found_beside_its_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let alpha = write(root, "crates/example/tests/alpha.rs", "mod support;\n");
    let support = write(root, "crates/example/tests/support/mod.rs", "mod inner;\n");
    let inner = write(
        root,
        "crates/example/tests/support/inner.rs",
        "#[test]\n#[ignore = \"requires independent clingo\"]\nfn nested_comparison() {}\n",
    );
    let found = comparisons(root, &[alpha, support, inner]).unwrap();
    assert_eq!(
        summary(&found),
        [(
            "example".into(),
            Some("alpha".into()),
            "nested_comparison".into()
        )]
    );
}

#[test]
fn a_comparison_outside_a_test_target_has_no_target() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let lib = write(
        root,
        "crates/example/src/lib.rs",
        "#[cfg(test)]\nmod tests {\n    #[test]\n    #[ignore = \"requires independent clingo\"]\n    fn unit_comparison() {}\n}\n",
    );
    let orphan = write(
        root,
        "crates/example/tests/support/orphan.rs",
        "#[test]\n#[ignore = \"requires independent clingo\"]\nfn orphan_comparison() {}\n",
    );
    let found = comparisons(root, &[lib, orphan]).unwrap();
    assert_eq!(
        summary(&found),
        [
            ("example".into(), None, "unit_comparison".into()),
            ("example".into(), None, "orphan_comparison".into()),
        ]
    );
}

#[test]
fn an_ignore_under_cfg_attr_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let alpha = write(
        root,
        "crates/example/tests/alpha.rs",
        "#[test]\n#[cfg_attr(unix, ignore = \"requires independent clingo\")]\nfn gated() {}\n",
    );
    assert!(comparisons(root, &[alpha]).is_err());
}

#[test]
fn a_declared_module_without_a_file_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let alpha = write(root, "crates/example/tests/alpha.rs", "mod missing;\n");
    assert!(comparisons(root, &[alpha]).is_err());
}
