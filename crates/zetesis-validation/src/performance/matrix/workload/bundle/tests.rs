use super::*;

struct Fixture {
    directory: tempfile::TempDir,
    files: Vec<BundleFile>,
    contract: examples::Contract,
}

impl Fixture {
    fn new() -> Self {
        let mut result = Self {
            directory: tempfile::tempdir().unwrap(),
            files: Vec::new(),
            contract: examples::Contract::complete_family(std::num::NonZeroU64::MIN),
        };
        result.file(
            "nested/entry.lp",
            "#include \"../base.lp\".\n#include \"../base.lp\".\np.\n",
            &[("../base.lp", "base.lp"), ("../base.lp", "base.lp")],
        );
        result.file("base.lp", "q.\n", &[]);
        result
    }

    fn file(&mut self, path: &str, text: &str, includes: &[(&str, &str)]) {
        let actual = self.directory.path().join(path);
        std::fs::create_dir_all(actual.parent().unwrap()).unwrap();
        std::fs::write(&actual, text).unwrap();
        self.files.push(BundleFile {
            path: path.into(),
            sha256: digest::sha256(text.as_bytes()),
            includes: includes
                .iter()
                .map(|(spelling, target)| BundleInclude {
                    spelling: (*spelling).into(),
                    target: (*target).into(),
                })
                .collect(),
        });
    }

    fn admit(&self, limits: WorkloadLimits) -> Result<Workload, Error> {
        Workload::bundle(
            BundleProgram {
                root: self.directory.path(),
                entry: "nested/entry.lp",
                files: &self.files,
                contract: &self.contract,
            },
            limits,
        )
    }
}

#[test]
fn copies_original_graph_and_repeated_include_order() {
    let fixture = Fixture::new();
    let workload = fixture.admit(WorkloadLimits::default()).unwrap();
    assert!(workload.is_bundle());
    assert!(!workload.is_amended());
    assert_eq!(workload.contract(), Some(&fixture.contract));
    let directory = tempfile::tempdir().unwrap();
    let seals = workload.materialize_bundle(directory.path(), 1024).unwrap();
    workload
        .validate_bundle_directory(directory.path(), false)
        .unwrap();
    assert_eq!(seals.len(), fixture.files.len());
    for file in &fixture.files {
        assert_eq!(
            std::fs::read(fixture.directory.path().join(&file.path)).unwrap(),
            std::fs::read(directory.path().join(&file.path)).unwrap()
        );
    }
    let record = serde_json::to_value(&workload).unwrap();
    assert_eq!(
        record["bundle"]["files"][0]["includes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(record.get("manifest_sha256").is_none());
    assert!(record.get("authored").is_none());
}

#[test]
fn graph_identity_excludes_root_and_member_order() {
    let first = Fixture::new();
    let mut second = Fixture::new();
    second.files.reverse();
    let one = first.admit(WorkloadLimits::default()).unwrap();
    let two = second.admit(WorkloadLimits::default()).unwrap();
    assert_eq!(one.identity(), two.identity());
    second.contract = examples::Contract::complete_family(std::num::NonZeroU64::new(2).unwrap());
    assert_ne!(
        one.identity(),
        second.admit(WorkloadLimits::default()).unwrap().identity()
    );
}

#[test]
fn graph_members_are_exactly_the_reachable_closure() {
    let mut fixture = Fixture::new();
    fixture.files.pop();
    assert!(fixture.admit(WorkloadLimits::default()).is_err());
    let mut fixture = Fixture::new();
    fixture.file("unused.lp", "unused.", &[]);
    assert!(fixture.admit(WorkloadLimits::default()).is_err());
    let mut fixture = Fixture::new();
    fixture.files.push(fixture.files[1].clone());
    assert!(fixture.admit(WorkloadLimits::default()).is_err());
}

#[test]
fn parsed_include_occurrences_must_match_the_manifest() {
    let mut fixture = Fixture::new();
    fixture.files[0].includes.pop();
    assert!(fixture.admit(WorkloadLimits::default()).is_err());
    let mut fixture = Fixture::new();
    fixture.files[0].includes[0].spelling = "../different.lp".into();
    assert!(fixture.admit(WorkloadLimits::default()).is_err());
}

#[test]
fn input_and_identical_copy_charge_the_source_allowance() {
    let fixture = Fixture::new();
    let admitted = fixture.admit(WorkloadLimits::default()).unwrap();
    let bytes: usize = admitted
        .sources
        .iter()
        .map(|source| source.derived_bytes)
        .sum();
    let limits = WorkloadLimits {
        total_source_bytes: bytes * 2,
        ..WorkloadLimits::default()
    };
    assert!(fixture.admit(limits).is_ok());
    assert!(
        fixture
            .admit(WorkloadLimits {
                total_source_bytes: bytes * 2 - 1,
                ..limits
            })
            .is_err()
    );
    assert!(
        fixture
            .admit(WorkloadLimits {
                metadata_bytes: admitted.metadata_bytes - 1,
                ..limits
            })
            .is_err()
    );
}

#[test]
fn changed_original_member_prevents_copying() {
    let fixture = Fixture::new();
    let admitted = fixture.admit(WorkloadLimits::default()).unwrap();
    std::fs::write(fixture.directory.path().join("base.lp"), "r.\n").unwrap();
    assert!(
        admitted
            .validate_bundle(crate::performance::Limits::default())
            .is_err()
    );
    let copied = tempfile::tempdir().unwrap();
    assert!(admitted.materialize_bundle(copied.path(), 1024).is_err());
}

#[test]
fn cwd_shadow_is_refused_even_with_identical_bytes() {
    let fixture = Fixture::new();
    let admitted = fixture.admit(WorkloadLimits::default()).unwrap();
    let campaign = tempfile::tempdir().unwrap();
    let copied = campaign.path().join("workload-00");
    admitted.materialize_bundle(&copied, 1024).unwrap();
    admitted.validate_bundle_directory(&copied, false).unwrap();
    std::fs::write(campaign.path().join("base.lp"), "q.\n").unwrap();
    assert!(matches!(
        admitted.validate_bundle_directory(&copied, false),
        Err(Error::Configuration(
            "cwd include shadows the declared bundle target"
        ))
    ));
}

#[test]
fn cwd_directory_shadow_is_a_refusal() {
    let fixture = Fixture::new();
    let admitted = fixture.admit(WorkloadLimits::default()).unwrap();
    let campaign = tempfile::tempdir().unwrap();
    let copied = campaign.path().join("workload-00");
    admitted.materialize_bundle(&copied, 1024).unwrap();
    std::fs::create_dir(campaign.path().join("base.lp")).unwrap();
    assert!(admitted.validate_bundle_directory(&copied, false).is_err());
}

#[test]
fn declared_parent_escape_and_cycles_are_refused() {
    let mut fixture = Fixture::new();
    fixture.files[0].includes[0].spelling = "../../base.lp".into();
    assert!(fixture.admit(WorkloadLimits::default()).is_err());
    let mut fixture = Fixture::new();
    fixture.files[1].includes.push(BundleInclude {
        spelling: "nested/entry.lp".into(),
        target: "nested/entry.lp".into(),
    });
    assert!(fixture.admit(WorkloadLimits::default()).is_err());
}

#[test]
fn cached_dag_branches_preserve_the_longest_depth_bound() {
    let hash = digest::sha256(b"p.");
    let mut files: Vec<_> = (0..33)
        .map(|index| BundleFile {
            path: format!("{index}.lp"),
            sha256: hash.clone(),
            includes: Vec::new(),
        })
        .collect();
    for (index, file) in files.iter_mut().enumerate().take(32).skip(1) {
        file.includes.push(BundleInclude {
            spelling: format!("{}.lp", index + 1),
            target: format!("{}.lp", index + 1),
        });
    }
    // Visit the deep branch through a short path first, then through node 1.
    for child in [2, 1] {
        files[0].includes.push(BundleInclude {
            spelling: format!("{child}.lp"),
            target: format!("{child}.lp"),
        });
    }
    assert!(matches!(
        graph::validate("0.lp", &files),
        Err(Error::Configuration("bundle include depth exceeds 32"))
    ));
    files[0].includes.pop();
    files.remove(1);
    assert!(graph::validate("0.lp", &files).is_ok());
}

#[test]
fn external_campaigns_require_fresh_reference_qualification() {
    let fixture = Fixture::new();
    let workload = fixture.admit(WorkloadLimits::default()).unwrap();
    assert!(workload.validate_bundle_campaign(true, false).is_ok());
    assert!(matches!(
        workload.validate_bundle_campaign(false, false),
        Err(Error::Configuration(
            "external bundles require fresh reference qualification"
        ))
    ));
}

#[test]
fn memory_record_names_cannot_replace_bundle_members() {
    let mut fixture = Fixture::new();
    fixture.files.clear();
    fixture.file("child-rss.json", "p.", &[]);
    let workload = Workload::bundle(
        BundleProgram {
            root: fixture.directory.path(),
            entry: "child-rss.json",
            files: &fixture.files,
            contract: &fixture.contract,
        },
        WorkloadLimits::default(),
    )
    .unwrap();
    assert!(workload.validate_bundle_campaign(true, false).is_ok());
    assert!(matches!(
        workload.validate_bundle_campaign(true, true),
        Err(Error::Configuration(
            "bundle member conflicts with the memory helper record"
        ))
    ));
}

#[cfg(unix)]
#[test]
fn member_symlinks_cannot_escape_the_root() {
    let fixture = Fixture::new();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("base.lp"), "q.\n").unwrap();
    std::fs::remove_file(fixture.directory.path().join("base.lp")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("base.lp"),
        fixture.directory.path().join("base.lp"),
    )
    .unwrap();
    assert!(matches!(
        fixture.admit(WorkloadLimits::default()),
        Err(Error::Corpus(examples::Error::Path(_)))
    ));
}

#[cfg(unix)]
#[test]
fn distinct_member_names_cannot_alias_one_canonical_file() {
    let mut fixture = Fixture::new();
    fixture.files.clear();
    fixture.file(
        "nested/entry.lp",
        "#include \"../base.lp\". #include \"../alias.lp\".",
        &[("../base.lp", "base.lp"), ("../alias.lp", "alias.lp")],
    );
    fixture.file("base.lp", "q.", &[]);
    std::os::unix::fs::symlink("base.lp", fixture.directory.path().join("alias.lp")).unwrap();
    fixture.files.push(BundleFile {
        path: "alias.lp".into(),
        sha256: digest::sha256(b"q."),
        includes: Vec::new(),
    });
    assert!(matches!(
        fixture.admit(WorkloadLimits::default()),
        Err(Error::Configuration(
            "bundle members alias the same canonical file"
        ))
    ));
}

#[test]
fn memory_record_check_uses_actual_materialized_filesystem() {
    let fixture = Fixture::new();
    let admitted = fixture.admit(WorkloadLimits::default()).unwrap();
    let copied = tempfile::tempdir().unwrap();
    admitted.materialize_bundle(copied.path(), 1024).unwrap();
    std::fs::write(copied.path().join("CHILD-RSS.JSON"), "p.").unwrap();
    let aliases = copied
        .path()
        .join("child-rss.json")
        .symlink_metadata()
        .is_ok();
    assert_eq!(
        admitted
            .validate_bundle_directory(copied.path(), true)
            .is_err(),
        aliases
    );
    std::fs::write(copied.path().join("child-rss.json"), "p.").unwrap();
    assert!(
        admitted
            .validate_bundle_directory(copied.path(), true)
            .is_err()
    );
}

#[test]
fn changed_manifest_prevents_campaign_admission() {
    let fixture = Fixture::new();
    let mut admitted = fixture.admit(WorkloadLimits::default()).unwrap();
    let path = fixture.directory.path().join("manifest.json");
    std::fs::write(&path, "original").unwrap();
    let seal = crate::selected::identity::seal(&path, 1024).unwrap();
    admitted
        .attach_bundle_manifest(&seal, WorkloadLimits::default().metadata_bytes)
        .unwrap();
    assert!(
        admitted
            .validate_bundle(crate::performance::Limits::default())
            .is_ok()
    );
    std::fs::write(&path, "modified").unwrap();
    assert!(
        admitted
            .validate_bundle(crate::performance::Limits::default())
            .is_err()
    );
}
