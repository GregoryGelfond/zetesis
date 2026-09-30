//! Every ignored test names the one resource it requires, and the gate that
//! supplies the resource selects it. A test is ignored only when a plain
//! `cargo test` cannot assume what it needs: its reason begins
//! `requires clingo:`, `requires Metal:` or `requires Vulkan:`, followed by
//! what it establishes. `scripts/check.sh oracle` runs the clingo tests its
//! campaigns select by hand, each by test target and, within a crate's
//! integration target, by module filter; the hardware gate runs each backend's
//! reviewed selection by exact name. A test no gate selects is run by nothing.

mod oracle_campaigns;
mod sources;

use std::fs;
use std::path::{Path, PathBuf};

use oracle_campaigns::{Campaign, campaigns};
use sources::{Ignored, Target, ignored};
use zetesis_backend::GpuApi;
use zetesis_maintenance::coverage;

use crate::support::{authored_sources, repository};

/// The resource an ignored test requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Resource {
    Clingo,
    Metal,
    Vulkan,
}

impl Resource {
    /// The resource `reason` names: the vocabulary's prefix, followed by a
    /// statement of what the test establishes.
    fn named(reason: &str) -> Option<Self> {
        let (prefix, statement) = reason.split_once(": ")?;
        let resource = match prefix {
            "requires clingo" => Self::Clingo,
            "requires Metal" => Self::Metal,
            "requires Vulkan" => Self::Vulkan,
            _ => return None,
        };
        (!statement.trim().is_empty()).then_some(resource)
    }
}

/// One test a reviewed hardware selection names: its target and exact name.
type Entry = (Target, String);

/// A target as `cargo test` selects it.
fn selecting(target: &Target) -> String {
    match target {
        Target::Lib => "--lib".into(),
        Target::Test(name) => format!("--test {name}"),
    }
}

/// The tests whose ignore names no resource in the vocabulary.
fn unstated(found: &[Ignored]) -> Vec<&Ignored> {
    found
        .iter()
        .filter(|test| Resource::named(&test.reason).is_none())
        .collect()
}

/// The clingo tests no campaign runs.
fn unselected<'a>(found: &'a [Ignored], campaigns: &[Campaign]) -> Vec<&'a Ignored> {
    found
        .iter()
        .filter(|test| Resource::named(&test.reason) == Some(Resource::Clingo))
        .filter(|test| !runs(campaigns, test))
        .collect()
}

/// Whether a campaign runs `test`; no campaign selects a library target.
fn runs(campaigns: &[Campaign], test: &Ignored) -> bool {
    matches!(&test.target, Some(Target::Test(target)) if campaigns
        .iter()
        .any(|campaign| campaign.runs(&test.package, target, &test.name)))
}

/// The tests requiring `resource` that its selection does not list.
fn unlisted<'a>(found: &'a [Ignored], resource: Resource, selection: &[Entry]) -> Vec<&'a Ignored> {
    found
        .iter()
        .filter(|test| Resource::named(&test.reason) == Some(resource))
        .filter(|test| {
            !test.target.as_ref().is_some_and(|target| {
                selection
                    .iter()
                    .any(|(listed, name)| listed == target && *name == test.name)
            })
        })
        .collect()
}

/// The entries of the selection for `resource` that name no test requiring it.
fn idle<'a>(found: &[Ignored], resource: Resource, selection: &'a [Entry]) -> Vec<&'a Entry> {
    selection
        .iter()
        .filter(|(target, name)| {
            !found.iter().any(|test| {
                Resource::named(&test.reason) == Some(resource)
                    && test.target.as_ref() == Some(target)
                    && test.name == *name
            })
        })
        .collect()
}

/// The tests, one per line, each with its reason.
fn listing(tests: &[&Ignored]) -> String {
    tests
        .iter()
        .map(|test| format!("{test}: {:?}", test.reason))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The names of `tests`, in order.
fn names<'a>(tests: &[&'a Ignored]) -> Vec<&'a str> {
    tests.iter().map(|test| test.name.as_str()).collect()
}

/// The ignored tests among the repository's maintained sources.
fn repository_tests(root: &Path) -> Vec<Ignored> {
    ignored(root, &authored_sources(root)).unwrap()
}

fn oracle_gate(root: &Path) -> Vec<Campaign> {
    let script = fs::read_to_string(root.join("scripts/check.sh")).unwrap();
    campaigns(&script).unwrap()
}

/// The reviewed hardware selections the hardware gate reads, with the
/// resource each qualifies.
fn hardware_gate(root: &Path) -> [(Resource, Vec<Entry>); 2] {
    [
        (Resource::Metal, GpuApi::Metal, "physical-selection.txt"),
        (
            Resource::Vulkan,
            GpuApi::Vulkan,
            "physical-selection-vulkan.txt",
        ),
    ]
    .map(|(resource, api, table)| {
        let path = root
            .join("crates/zetesis-maintenance/src/coverage")
            .join(table);
        let selection = coverage::selection(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(selection.api, api, "{table}");
        let entries = selection
            .groups
            .iter()
            .flat_map(|group| {
                let target = if group.target_kind == "lib" {
                    Target::Lib
                } else {
                    Target::Test(group.target.clone())
                };
                group
                    .tests
                    .iter()
                    .map(move |test| (target.clone(), test.clone()))
            })
            .collect();
        (resource, entries)
    })
}

#[test]
fn every_ignored_test_names_its_resource() {
    let root = repository();
    let found = repository_tests(&root);
    assert!(
        !found.is_empty(),
        "the maintained sources hold ignored tests"
    );
    let unstated = unstated(&found);
    assert!(
        unstated.is_empty(),
        "ignored tests whose reason names no resource:\n{}",
        listing(&unstated)
    );
    println!("ignored_tests={}", found.len());
}

#[test]
fn the_oracle_gate_runs_every_clingo_comparison() {
    let root = repository();
    let gate = oracle_gate(&root);
    let found = repository_tests(&root);
    let unrun = unselected(&found, &gate);
    assert!(
        unrun.is_empty(),
        "clingo tests no oracle campaign runs:\n{}",
        listing(&unrun)
    );
}

/// The clingo tests among `found`, with the target that compiles each.
fn clingo_tests(found: &[Ignored]) -> Vec<(&Ignored, &str)> {
    found
        .iter()
        .filter(|test| Resource::named(&test.reason) == Some(Resource::Clingo))
        .filter_map(|test| match &test.target {
            Some(Target::Test(target)) => Some((test, target.as_str())),
            _ => None,
        })
        .collect()
}

#[test]
fn every_target_the_oracle_gate_names_holds_a_clingo_comparison() {
    let root = repository();
    let gate = oracle_gate(&root);
    let found = repository_tests(&root);
    let tests = clingo_tests(&found);
    // A target is named together with its campaign's filter: with one
    // integration target per crate, the filter is what selects the tests.
    let idle: Vec<String> = gate
        .iter()
        .flat_map(|campaign| {
            campaign
                .targets
                .iter()
                .map(move |target| (campaign, target))
        })
        .filter(|(campaign, target)| {
            !tests.iter().any(|(test, compiled)| {
                *compiled == target.as_str() && campaign.runs(&test.package, target, &test.name)
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
        "oracle campaign targets holding no clingo test:\n{}",
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
    let found = repository_tests(&root);
    let tests = clingo_tests(&found);
    let idle: Vec<String> = gate
        .iter()
        .flat_map(|campaign| {
            campaign
                .filters
                .iter()
                .map(move |filter| (campaign, filter))
        })
        .filter(|(campaign, filter)| {
            !tests.iter().any(|(test, target)| {
                test.name.contains(filter.as_str())
                    && campaign.runs(&test.package, target, &test.name)
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
        "oracle campaign filters selecting no clingo test:\n{}",
        idle.join("\n")
    );
}

#[test]
fn every_hardware_test_is_in_its_backends_selection() {
    let root = repository();
    let found = repository_tests(&root);
    for (resource, selection) in hardware_gate(&root) {
        let unlisted = unlisted(&found, resource, &selection);
        assert!(
            unlisted.is_empty(),
            "{resource:?} tests its reviewed selection does not list:\n{}",
            listing(&unlisted)
        );
    }
}

#[test]
fn every_selection_entry_names_a_hardware_test() {
    let root = repository();
    let found = repository_tests(&root);
    for (resource, selection) in hardware_gate(&root) {
        let idle: Vec<String> = idle(&found, resource, &selection)
            .into_iter()
            .map(|(target, name)| format!("{}: {name}", selecting(target)))
            .collect();
        assert!(
            idle.is_empty(),
            "{resource:?} selection entries naming no {resource:?} test:\n{}",
            idle.join("\n")
        );
    }
}

#[test]
fn a_reason_names_its_resource_by_the_vocabulary() {
    for (reason, resource) in [
        ("requires clingo: the answers agree", Some(Resource::Clingo)),
        ("requires Metal: the answers agree", Some(Resource::Metal)),
        ("requires Vulkan: the answers agree", Some(Resource::Vulkan)),
        ("requires clingo:", None),
        ("requires clingo: ", None),
        ("requires Clingo: the answers agree", None),
        ("requires an independently installed clingo", None),
        (
            "requires actual Metal; explicit physical qualification",
            None,
        ),
        ("slow", None),
        ("", None),
    ] {
        assert_eq!(Resource::named(reason), resource, "{reason:?}");
    }
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

#[test]
fn an_ignore_without_a_resource_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let alpha = write(
        root,
        "crates/example/tests/alpha.rs",
        r#"#[test]
#[ignore]
fn bare() {}
#[test]
#[ignore = "slow"]
fn unexplained() {}
#[test]
#[ignore = "requires an independently installed clingo"]
fn another_spelling() {}
#[test]
#[ignore = "requires Metal:"]
fn unstated() {}
#[test]
#[ignore = "requires clingo: the fixture agrees with clingo"]
fn stated() {}
"#,
    );
    let found = ignored(root, &[alpha]).unwrap();
    assert_eq!(
        names(&unstated(&found)),
        ["bare", "unexplained", "another_spelling", "unstated"]
    );
}

#[test]
fn a_clingo_test_no_campaign_selects_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let comparison = "#[test]\n#[ignore = \"requires clingo: the fixture agrees with clingo\"]\nfn comparison() {}\n";
    let sources = [
        write(
            root,
            "crates/example/tests/integration/main.rs",
            "mod alpha;\nmod beta;\n",
        ),
        write(
            root,
            "crates/example/tests/integration/alpha.rs",
            comparison,
        ),
        write(root, "crates/example/tests/integration/beta.rs", comparison),
        write(root, "crates/example/src/lib.rs", comparison),
    ];
    let found = ignored(root, &sources).unwrap();
    let gate =
        campaigns("oracle_test --locked -p example --test integration -- --ignored alpha::\n")
            .unwrap();
    // No campaign selects a library target: the oracle gate runs integration
    // tests only.
    assert_eq!(
        names(&unselected(&found, &gate)),
        ["comparison", "beta::comparison"]
    );
}

/// A package whose library holds three Metal tests and a Vulkan one.
fn hardware_fixture(root: &Path) -> Vec<Ignored> {
    package(root, "crates/example", "example");
    let sources = [
        write(root, "crates/example/src/lib.rs", "mod device;\n"),
        write(
            root,
            "crates/example/src/device.rs",
            r#"#[test]
#[ignore = "requires Metal: the listed test runs"]
fn metal_listed() {}
#[test]
#[ignore = "requires Metal: the unlisted test runs"]
fn metal_unlisted() {}
#[test]
#[ignore = "requires Vulkan: the Vulkan test runs"]
fn vulkan_listed() {}
"#,
        ),
    ];
    ignored(root, &sources).unwrap()
}

#[test]
fn a_hardware_test_no_selection_lists_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let found = hardware_fixture(directory.path());
    let selection = [
        (Target::Lib, "device::metal_listed".to_owned()),
        (Target::Lib, "device::vulkan_listed".to_owned()),
    ];
    assert_eq!(
        names(&unlisted(&found, Resource::Metal, &selection)),
        ["device::metal_unlisted"]
    );
    assert!(unlisted(&found, Resource::Vulkan, &selection).is_empty());
}

#[test]
fn a_selection_entry_naming_no_test_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let found = hardware_fixture(directory.path());
    let selection = [
        (Target::Lib, "device::metal_listed".to_owned()),
        (Target::Lib, "device::metal_unlisted".to_owned()),
        (Target::Lib, "device::metal_missing".to_owned()),
        (
            Target::Test("integration".into()),
            "device::metal_listed".to_owned(),
        ),
        (Target::Lib, "device::vulkan_listed".to_owned()),
    ];
    assert_eq!(
        idle(&found, Resource::Metal, &selection),
        [&selection[2], &selection[3], &selection[4]]
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

fn summary(found: &[Ignored]) -> Vec<(String, Option<Target>, String)> {
    found
        .iter()
        .map(|test| (test.package.clone(), test.target.clone(), test.name.clone()))
        .collect()
}

const IGNORED: &str =
    "#[test]\n#[ignore = \"requires clingo: the fixture agrees with clingo\"]\nfn ";

#[test]
fn an_ignored_test_belongs_to_each_target_that_includes_its_module() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let include = "#[path = \"support/shared.rs\"]\nmod shared;\n";
    let alpha = write(root, "crates/example/tests/alpha.rs", include);
    let beta = write(root, "crates/example/tests/beta.rs", include);
    let shared = write(
        root,
        "crates/example/tests/support/shared.rs",
        &format!("{IGNORED}shared_test() {{}}\n"),
    );
    let found = ignored(root, &[alpha, beta, shared]).unwrap();
    assert_eq!(
        summary(&found),
        [
            (
                "example".into(),
                Some(Target::Test("alpha".into())),
                "shared::shared_test".into()
            ),
            (
                "example".into(),
                Some(Target::Test("beta".into())),
                "shared::shared_test".into()
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
        &format!("{IGNORED}comparison() {{}}\n"),
    );
    let found = ignored(root, &[main, alpha]).unwrap();
    assert_eq!(
        summary(&found),
        [(
            "example".into(),
            Some(Target::Test("integration".into())),
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
        &format!("{IGNORED}nested_test() {{}}\n"),
    );
    let found = ignored(root, &[alpha, support, inner]).unwrap();
    assert_eq!(
        summary(&found),
        [(
            "example".into(),
            Some(Target::Test("alpha".into())),
            "support::inner::nested_test".into()
        )]
    );
}

#[test]
fn a_library_test_belongs_to_the_library_target() {
    // The library's harness names a unit test by its module path from
    // `src/lib.rs`; a module's children lie under the file's own directory.
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let sources = [
        write(
            root,
            "crates/example/src/lib.rs",
            &format!(
                "mod device;\n#[cfg(test)]\nmod tests {{\n    {IGNORED}unit_test() {{}}\n}}\n"
            ),
        ),
        write(root, "crates/example/src/device.rs", "mod tests;\n"),
        write(
            root,
            "crates/example/src/device/tests.rs",
            &format!("{IGNORED}device_test() {{}}\n"),
        ),
    ];
    let found = ignored(root, &sources).unwrap();
    assert_eq!(
        summary(&found),
        [
            (
                "example".into(),
                Some(Target::Lib),
                "device::tests::device_test".into()
            ),
            (
                "example".into(),
                Some(Target::Lib),
                "tests::unit_test".into()
            ),
        ]
    );
}

#[test]
fn a_test_outside_every_target_has_no_target() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let orphan = write(
        root,
        "crates/example/tests/support/orphan.rs",
        &format!("{IGNORED}orphan_test() {{}}\n"),
    );
    let found = ignored(root, &[orphan]).unwrap();
    assert_eq!(
        summary(&found),
        [("example".into(), None, "orphan_test".into())]
    );
}

#[test]
fn an_ignored_function_that_is_not_a_test_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let alpha = write(
        root,
        "crates/example/tests/alpha.rs",
        "#[ignore = \"requires clingo: the fixture agrees with clingo\"]\nfn helper() {}\n",
    );
    assert!(ignored(root, &[alpha]).is_err());
}

#[test]
fn an_ignore_under_cfg_attr_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let alpha = write(
        root,
        "crates/example/tests/alpha.rs",
        "#[test]\n#[cfg_attr(unix, ignore = \"requires clingo: the fixture agrees\")]\nfn gated() {}\n",
    );
    assert!(ignored(root, &[alpha]).is_err());
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
    assert!(ignored(root, &[alpha]).is_err());
}

#[test]
fn a_declared_module_without_a_file_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    package(root, "crates/example", "example");
    let alpha = write(root, "crates/example/tests/alpha.rs", "mod missing;\n");
    assert!(ignored(root, &[alpha]).is_err());
}
