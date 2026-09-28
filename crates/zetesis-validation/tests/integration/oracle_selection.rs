//! Every comparison against clingo in the maintained sources is run by the
//! oracle gate. Such a test is ignored, since the oracle is external, and
//! `scripts/check.sh oracle` runs the ignored tests its campaigns select by
//! hand, each by test target and, within a crate's integration target, by
//! module filter; a test no campaign selects is run by nothing.

use crate::support::authored_sources;
mod clingo_comparisons;
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
    // A target is named together with its campaign's filter: with one
    // integration target per crate, the filter is what selects the comparisons.
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
                comparison.target.as_deref() == Some(target.as_str())
                    && campaign.runs(&comparison.package, target, &comparison.name)
            })
        })
        .map(|(campaign, target)| {
            let filters: String = campaign
                .filters
                .iter()
                .flat_map(|filter| [" ", filter.as_str()])
                .collect();
            format!(
                "scripts/check.sh:{}: -p {} --test {target}{filters}",
                campaign.line, campaign.package
            )
        })
        .collect();
    assert!(
        idle.is_empty(),
        "oracle campaign targets holding no clingo comparison:\n{}",
        idle.join("\n")
    );
}

#[test]
fn every_filter_the_oracle_gate_names_selects_a_clingo_comparison() {
    // Within a crate's integration target a filter names a module the gate
    // runs, as a target once did; a filter left behind by a rename selects
    // nothing while its campaign's other filters keep the target busy.
    let root = repository();
    let gate = oracle_gate(&root);
    let sources = authored_sources::inventory(&root).unwrap();
    let found = comparisons(&root, &sources).unwrap();
    let idle: Vec<String> = gate
        .iter()
        .flat_map(|campaign| {
            campaign
                .filters
                .iter()
                .map(move |filter| (campaign, filter))
        })
        .filter(|(campaign, filter)| {
            !found.iter().any(|comparison| {
                comparison.name.contains(filter.as_str())
                    && comparison.target.as_deref().is_some_and(|target| {
                        campaign.runs(&comparison.package, target, &comparison.name)
                    })
            })
        })
        .map(|(campaign, filter)| {
            format!(
                "scripts/check.sh:{}: -p {} {filter}",
                campaign.line, campaign.package
            )
        })
        .collect();
    assert!(
        idle.is_empty(),
        "oracle campaign filters selecting no clingo comparison:\n{}",
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
    assert!(found[0].filters.is_empty());
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
    assert_eq!(found[0].filters, ["original_sources_agree"]);
    assert!(found[0].runs("example", "alpha", "original_sources_agree"));
    assert!(found[0].runs("example", "alpha", "the_original_sources_agree_with_clingo"));
    assert!(!found[0].runs("example", "alpha", "another_comparison"));
}

#[test]
fn a_campaign_filter_matches_the_module_path() {
    // libtest matches a filter against the whole name, module path included,
    // so a module's name selects every test inside it.
    let script = "oracle_test --locked -p example --test integration alpha:: -- --ignored\n";
    let found = campaigns(script).unwrap();
    assert!(found[0].runs("example", "integration", "alpha::comparison"));
    assert!(!found[0].runs("example", "integration", "beta::comparison"));
}

#[test]
fn harness_filters_select_the_tests_any_of_them_names() {
    // libtest takes several filters after `--` and runs a test matching any.
    let script = "oracle_test --locked -p example --test integration -- --ignored alpha:: beta::\n";
    let found = campaigns(script).unwrap();
    assert_eq!(found[0].filters, ["alpha::", "beta::"]);
    assert!(found[0].runs("example", "integration", "alpha::comparison"));
    assert!(found[0].runs("example", "integration", "beta::comparison"));
    assert!(!found[0].runs("example", "integration", "gamma::comparison"));
}

#[test]
fn a_harness_option_the_gate_does_not_use_is_refused() {
    let script = "oracle_test --locked -p example --test alpha -- --ignored --exact\n";
    assert!(campaigns(script).is_err());
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
                "shared::shared_comparison".into()
            ),
            (
                "example".into(),
                Some("beta".into()),
                "shared::shared_comparison".into()
            ),
        ]
    );
}

#[test]
fn a_directory_target_is_named_by_its_directory() {
    // Cargo builds `tests/NAME/main.rs` as the test target NAME.
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let main = write(
        root,
        "crates/example/tests/integration/main.rs",
        "mod alpha;\n",
    );
    let alpha = write(
        root,
        "crates/example/tests/integration/alpha.rs",
        "#[test]\n#[ignore = \"requires independent clingo\"]\nfn comparison() {}\n",
    );
    let found = comparisons(root, &[main, alpha]).unwrap();
    assert_eq!(
        summary(&found),
        [(
            "example".into(),
            Some("integration".into()),
            "alpha::comparison".into()
        )]
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
            "support::inner::nested_comparison".into()
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
            ("example".into(), None, "tests::unit_comparison".into()),
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
fn circular_module_declarations_are_refused() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let alpha = write(
        root,
        "crates/example/tests/alpha.rs",
        "#[path = \"alpha.rs\"]\nmod again;\n",
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
