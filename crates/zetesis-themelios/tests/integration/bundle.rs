//! Original include-graph loading without source rewriting or an ASP engine.

use std::fs;
use std::path::{Path, PathBuf};

use themelios_base::source::{SourceId, Sources, check_sources_laws};
use zetesis_themelios::{
    AdmissionOptions, BundleError, BundleLimits, BundleResource, BundleSource, SourceBundle, admit,
};

struct Fixture {
    directory: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().expect("temporary fixture directory"),
        }
    }
    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.directory.path().join(name);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture parents");
        fs::write(&path, bytes).expect("fixture contents");
        path
    }
    fn path(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }
    fn load(&self, name: &str) -> Result<SourceBundle, BundleError> {
        SourceBundle::load(self.path(name), BundleLimits::default())
    }
}

#[test]
fn relative_includes_keep_original_bytes_and_source_qualified_edges() {
    let fixture = Fixture::new();
    let entry_text = "% original\r\n#include \"parts/child.lp\".\r\np.\r\n";
    let child_text = "#include \"../leaf.lp\".\nq.\n";
    fixture.write("entry.lp", entry_text);
    fixture.write("parts/child.lp", child_text);
    fixture.write("leaf.lp", "r.\n");
    let bundle = fixture.load("entry.lp").expect("acyclic source graph");
    assert_eq!(bundle.sources().len(), 3);
    assert_eq!(bundle.entry(), SourceId::new(0));
    let entry = bundle.get(bundle.entry()).expect("entry");
    assert_eq!(entry.source().text(), entry_text);
    assert_eq!(entry.parsed().source(), entry.id());
    let edge = &entry.includes()[0];
    assert_eq!(edge.requested_path(), "parts/child.lp");
    assert_eq!(edge.location().source, entry.id());
    assert_eq!(
        entry
            .source()
            .slice(edge.location().span)
            .expect("original span"),
        "#include \"parts/child.lp\"."
    );
    let child = bundle.get(edge.target()).expect("child");
    assert_eq!(child.source().text(), child_text);
    assert_eq!(child.includes()[0].requested_path(), "../leaf.lp");
    assert_eq!(
        bundle.total_bytes(),
        entry_text.len() + child_text.len() + 3
    );
    let ids: Vec<_> = bundle
        .sources()
        .iter()
        .map(BundleSource::id)
        .chain([SourceId::new(999)])
        .collect();
    assert!(check_sources_laws(&bundle, &ids).is_empty());
    assert!(bundle.name(SourceId::new(999)).is_none());
    assert!(bundle.text(SourceId::new(999)).is_none());
    assert!(bundle.line_index(SourceId::new(999)).is_none());
}

#[test]
fn repeated_targets_retain_edges_without_copying_original_sources() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "#include \"shared.lp\". #include \"./shared.lp\".",
    );
    fixture.write("shared.lp", "s.");
    let bundle = fixture.load("entry.lp").expect("shared source");
    assert_eq!(bundle.sources().len(), 2);
    let edges = bundle.sources()[0].includes();
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].target(), edges[1].target());
    assert_ne!(edges[0].location(), edges[1].location());
}

#[test]
fn canonical_alias_cycles_are_reported_with_the_closing_edge() {
    let fixture = Fixture::new();
    fixture.write("a.lp", "#include \"sub/b.lp\".");
    fixture.write("sub/b.lp", "#include \"../a.lp\".");
    match fixture.load("a.lp") {
        Err(BundleError::Cycle { chain, location }) => {
            assert_eq!(chain.len(), 3);
            assert_eq!(chain.first(), chain.last());
            assert_eq!(location.source, SourceId::new(1));
        }
        other => panic!("expected include cycle: {other:?}"),
    }
}

#[test]
fn file_and_depth_limits_are_independent() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"child.lp\".");
    fixture.write("child.lp", "c.");
    for (limits, expected) in [
        (
            BundleLimits {
                max_files: 1,
                ..BundleLimits::default()
            },
            BundleResource::Files,
        ),
        (
            BundleLimits {
                max_include_depth: 0,
                ..BundleLimits::default()
            },
            BundleResource::IncludeDepth,
        ),
    ] {
        assert!(
            matches!(SourceBundle::load(fixture.path("entry.lp"), limits), Err(BundleError::Limit { resource, including: Some(_), .. }) if resource == expected)
        );
    }
}

#[test]
fn shared_subtrees_do_not_evade_the_maximum_path_depth() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "#include \"shared.lp\". #include \"branch.lp\".",
    );
    fixture.write("shared.lp", "#include \"leaf.lp\".");
    fixture.write("leaf.lp", "leaf.");
    fixture.write("branch.lp", "#include \"shared.lp\".");
    let limits = BundleLimits {
        max_include_depth: 2,
        ..BundleLimits::default()
    };
    assert!(matches!(
        SourceBundle::load(fixture.path("entry.lp"), limits),
        Err(BundleError::Limit {
            resource: BundleResource::IncludeDepth,
            observed: 3,
            ..
        })
    ));
}

#[test]
fn byte_limits_precede_parsing_and_apply_across_the_graph() {
    let fixture = Fixture::new();
    fixture.write("oversize.lp", "deliberately invalid syntax");
    let small = BundleLimits {
        max_file_bytes: 2,
        ..BundleLimits::default()
    };
    assert!(matches!(
        SourceBundle::load(fixture.path("oversize.lp"), small),
        Err(BundleError::Limit {
            resource: BundleResource::FileBytes,
            ..
        })
    ));
    let entry = "#include \"child.lp\".";
    fixture.write("entry.lp", entry);
    fixture.write("child.lp", "child.");
    let total = BundleLimits {
        max_total_bytes: entry.len() + 5,
        ..BundleLimits::default()
    };
    assert!(matches!(
        SourceBundle::load(fixture.path("entry.lp"), total),
        Err(BundleError::Limit {
            resource: BundleResource::TotalBytes,
            ..
        })
    ));
}

#[test]
fn an_empty_entry_can_use_zero_byte_and_depth_budgets() {
    let fixture = Fixture::new();
    fixture.write("empty.lp", "");
    let limits = BundleLimits {
        max_roots: 1,
        max_files: 1,
        max_file_bytes: 0,
        max_total_bytes: 0,
        max_include_depth: 0,
    };
    let bundle = SourceBundle::load(fixture.path("empty.lp"), limits).expect("empty entry");
    assert_eq!(bundle.total_bytes(), 0);
    assert_eq!(bundle.sources().len(), 1);
    assert!(bundle.sources()[0].includes().is_empty());
}

#[test]
fn child_parse_failure_preserves_its_original_bytes_and_identity() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"broken.lp\".");
    fixture.write("broken.lp", "bad(.");
    match fixture.load("entry.lp") {
        Err(BundleError::Syntax {
            source,
            diagnostics,
            ..
        }) => {
            assert_eq!(source.text(), "bad(.");
            assert_eq!(source.id(), SourceId::new(1));
            assert!(!diagnostics.is_empty());
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.primary().source == source.id())
            );
        }
        other => panic!("expected child parse failure: {other:?}"),
    }
}

#[test]
fn missing_paths_and_invalid_utf8_are_typed_refusals() {
    let fixture = Fixture::new();
    fixture.write("entry.lp", "#include \"missing.lp\".");
    assert!(matches!(
        fixture.load("entry.lp"),
        Err(BundleError::Io {
            including: Some(_),
            ..
        })
    ));
    fixture.write("invalid.lp", [0xff, 0xfe]);
    assert!(matches!(
        fixture.load("invalid.lp"),
        Err(BundleError::Source { .. })
    ));
    assert!(matches!(
        SourceBundle::load(fixture.directory.path(), BundleLimits::default()),
        Err(BundleError::NotRegularFile { .. })
    ));
}

#[test]
fn includes_are_read_from_ast_and_their_paths_use_dialect_decoding() {
    let fixture = Fixture::new();
    fixture.write(
        "entry.lp",
        "% #include \"missing.lp\".\np(\"#include missing\").\n#include \"quote\\\"name.lp\".",
    );
    fixture.write("quote\"name.lp", "q.");
    let bundle = fixture.load("entry.lp").expect("escaped original filename");
    assert_eq!(bundle.sources().len(), 2);
    assert_eq!(
        bundle.sources()[0].includes()[0].requested_path(),
        "quote\"name.lp"
    );
}

#[test]
fn richer_parsing_does_not_claim_s0_admission_or_library_resolution() {
    let fixture = Fixture::new();
    let richer = "#const n=1. p :- #count{1:q} > 0.";
    fixture.write("richer.lp", richer);
    let bundle = fixture
        .load("richer.lp")
        .expect("syntax is supported by themelios");
    assert_eq!(bundle.sources()[0].source().text(), richer);
    assert!(admit(richer.to_owned(), AdmissionOptions::default()).is_err());
    fixture.write("library.lp", "#include <incmode>.");
    assert!(
        matches!(fixture.load("library.lp"), Err(BundleError::LibraryInclude { name, .. }) if name == "incmode")
    );
}

#[test]
fn absolute_include_paths_remain_absolute() {
    let fixture = Fixture::new();
    let child = fixture.write("child.lp", "child.");
    let absolute = child.to_str().expect("test path is UTF-8");
    fixture.write("entry.lp", format!("#include \"{absolute}\"."));
    let bundle = fixture.load("entry.lp").expect("absolute include");
    assert!(Path::new(bundle.sources()[0].includes()[0].requested_path()).is_absolute());
    assert_eq!(
        bundle.sources()[1].path(),
        child.canonicalize().expect("canonical child")
    );
}

// Snapshot of the correctness examples' cases and include closures, as
// examples/correctness/manifest.json records them.
// This deliberately uses no JSON/runtime dependency in the source boundary.
const CORPUS_CASES: &[(&str, &[&str])] = &[
    (
        "scenarios/equality-generalized-tsp/01-basic.lp",
        &[
            "scenarios/equality-generalized-tsp/01-basic.lp",
            "encodings/equality-generalized-tsp/egtsp.lp",
        ],
    ),
    (
        "scenarios/equality-generalized-tsp/02-larger.lp",
        &[
            "scenarios/equality-generalized-tsp/02-larger.lp",
            "encodings/equality-generalized-tsp/egtsp.lp",
        ],
    ),
    (
        "scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp",
        &[
            "scenarios/equality-generalized-tsp/03-unreachable-subset-unsat.lp",
            "encodings/equality-generalized-tsp/egtsp.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-01/01-basic.lp",
        &[
            "scenarios/shortest-path/variant-01/01-basic.lp",
            "encodings/shortest-path/variant-01.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-01/02-start-equals-end.lp",
        &[
            "scenarios/shortest-path/variant-01/02-start-equals-end.lp",
            "encodings/shortest-path/variant-01.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-01/03-zero-cost-detour.lp",
        &[
            "scenarios/shortest-path/variant-01/03-zero-cost-detour.lp",
            "encodings/shortest-path/variant-01.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-01/04-no-path.lp",
        &[
            "scenarios/shortest-path/variant-01/04-no-path.lp",
            "encodings/shortest-path/variant-01.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-01/05-multi-path.lp",
        &[
            "scenarios/shortest-path/variant-01/05-multi-path.lp",
            "encodings/shortest-path/variant-01.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-01/06-layered-dag.lp",
        &[
            "scenarios/shortest-path/variant-01/06-layered-dag.lp",
            "encodings/shortest-path/variant-01.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-01/07-cycles.lp",
        &[
            "scenarios/shortest-path/variant-01/07-cycles.lp",
            "encodings/shortest-path/variant-01.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-01/08-negative-weights.lp",
        &[
            "scenarios/shortest-path/variant-01/08-negative-weights.lp",
            "encodings/shortest-path/variant-01.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-02/01-basic.lp",
        &[
            "scenarios/shortest-path/variant-02/01-basic.lp",
            "encodings/shortest-path/variant-02.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-02/02-start-equals-end.lp",
        &[
            "scenarios/shortest-path/variant-02/02-start-equals-end.lp",
            "encodings/shortest-path/variant-02.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-02/03-before-forces-detour.lp",
        &[
            "scenarios/shortest-path/variant-02/03-before-forces-detour.lp",
            "encodings/shortest-path/variant-02.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-02/04-after-forces-extension.lp",
        &[
            "scenarios/shortest-path/variant-02/04-after-forces-extension.lp",
            "encodings/shortest-path/variant-02.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-02/05-ordering-unsat.lp",
        &[
            "scenarios/shortest-path/variant-02/05-ordering-unsat.lp",
            "encodings/shortest-path/variant-02.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-02/06-layered-dag-before.lp",
        &[
            "scenarios/shortest-path/variant-02/06-layered-dag-before.lp",
            "encodings/shortest-path/variant-02.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp",
        &[
            "scenarios/shortest-path/variant-02/07-layered-dag-before-after.lp",
            "encodings/shortest-path/variant-02.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp",
        &[
            "scenarios/shortest-path/variant-02/08-tie-break-under-ordering.lp",
            "encodings/shortest-path/variant-02.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-02/09-negative-weights.lp",
        &[
            "scenarios/shortest-path/variant-02/09-negative-weights.lp",
            "encodings/shortest-path/variant-02.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-03/01-basic.lp",
        &[
            "scenarios/shortest-path/variant-03/01-basic.lp",
            "encodings/shortest-path/variant-03.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-03/02-start-equals-end.lp",
        &[
            "scenarios/shortest-path/variant-03/02-start-equals-end.lp",
            "encodings/shortest-path/variant-03.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-03/03-budget-forces-detour.lp",
        &[
            "scenarios/shortest-path/variant-03/03-budget-forces-detour.lp",
            "encodings/shortest-path/variant-03.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp",
        &[
            "scenarios/shortest-path/variant-03/04-cost-at-cap-allowed.lp",
            "encodings/shortest-path/variant-03.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-03/05-budget-unsat.lp",
        &[
            "scenarios/shortest-path/variant-03/05-budget-unsat.lp",
            "encodings/shortest-path/variant-03.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-03/06-layered-dag-cap.lp",
        &[
            "scenarios/shortest-path/variant-03/06-layered-dag-cap.lp",
            "encodings/shortest-path/variant-03.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp",
        &[
            "scenarios/shortest-path/variant-03/07-layered-dag-tight-cap.lp",
            "encodings/shortest-path/variant-03.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp",
        &[
            "scenarios/shortest-path/variant-03/08-tie-break-under-cap.lp",
            "encodings/shortest-path/variant-03.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-03/09-negative-weights.lp",
        &[
            "scenarios/shortest-path/variant-03/09-negative-weights.lp",
            "encodings/shortest-path/variant-03.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-04/01-basic.lp",
        &[
            "scenarios/shortest-path/variant-04/01-basic.lp",
            "encodings/shortest-path/variant-04.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-04/02-start-equals-end.lp",
        &[
            "scenarios/shortest-path/variant-04/02-start-equals-end.lp",
            "encodings/shortest-path/variant-04.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp",
        &[
            "scenarios/shortest-path/variant-04/03-before-forces-detour-within-budget.lp",
            "encodings/shortest-path/variant-04.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp",
        &[
            "scenarios/shortest-path/variant-04/04-ordering-violates-budget.lp",
            "encodings/shortest-path/variant-04.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp",
        &[
            "scenarios/shortest-path/variant-04/05-after-and-budget-interact.lp",
            "encodings/shortest-path/variant-04.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp",
        &[
            "scenarios/shortest-path/variant-04/06-layered-dag-ordering-cap.lp",
            "encodings/shortest-path/variant-04.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-04/07-layered-dag-combined.lp",
        &[
            "scenarios/shortest-path/variant-04/07-layered-dag-combined.lp",
            "encodings/shortest-path/variant-04.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp",
        &[
            "scenarios/shortest-path/variant-04/08-tie-break-under-ordering-and-cap.lp",
            "encodings/shortest-path/variant-04.lp",
        ],
    ),
    (
        "scenarios/shortest-path/variant-04/09-negative-weights.lp",
        &[
            "scenarios/shortest-path/variant-04/09-negative-weights.lp",
            "encodings/shortest-path/variant-04.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-01/01-basic.lp",
        &[
            "scenarios/task-allocation/variant-01/01-basic.lp",
            "encodings/task-allocation/variant-01.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-01/02-agent-reuse.lp",
        &[
            "scenarios/task-allocation/variant-01/02-agent-reuse.lp",
            "encodings/task-allocation/variant-01.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-01/03-selective-compatibility.lp",
        &[
            "scenarios/task-allocation/variant-01/03-selective-compatibility.lp",
            "encodings/task-allocation/variant-01.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp",
        &[
            "scenarios/task-allocation/variant-01/04-no-compatible-agent-unsat.lp",
            "encodings/task-allocation/variant-01.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-01/05-larger-mix.lp",
        &[
            "scenarios/task-allocation/variant-01/05-larger-mix.lp",
            "encodings/task-allocation/variant-01.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-02/01-basic.lp",
        &[
            "scenarios/task-allocation/variant-02/01-basic.lp",
            "encodings/task-allocation/variant-02.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp",
        &[
            "scenarios/task-allocation/variant-02/02-makespan-tiebreak.lp",
            "encodings/task-allocation/variant-02.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-02/03-cost-dominates.lp",
        &[
            "scenarios/task-allocation/variant-02/03-cost-dominates.lp",
            "encodings/task-allocation/variant-02.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp",
        &[
            "scenarios/task-allocation/variant-02/04-no-compatible-agent-unsat.lp",
            "encodings/task-allocation/variant-02.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-02/05-larger-mix.lp",
        &[
            "scenarios/task-allocation/variant-02/05-larger-mix.lp",
            "encodings/task-allocation/variant-02.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-03/01-basic.lp",
        &[
            "scenarios/task-allocation/variant-03/01-basic.lp",
            "encodings/task-allocation/variant-03.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-03/02-multiple-groups.lp",
        &[
            "scenarios/task-allocation/variant-03/02-multiple-groups.lp",
            "encodings/task-allocation/variant-03.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp",
        &[
            "scenarios/task-allocation/variant-03/03-mixed-grouped-ungrouped.lp",
            "encodings/task-allocation/variant-03.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp",
        &[
            "scenarios/task-allocation/variant-03/04-incompatible-group-unsat.lp",
            "encodings/task-allocation/variant-03.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-03/05-larger-mix.lp",
        &[
            "scenarios/task-allocation/variant-03/05-larger-mix.lp",
            "encodings/task-allocation/variant-03.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-04/01-basic.lp",
        &[
            "scenarios/task-allocation/variant-04/01-basic.lp",
            "encodings/task-allocation/variant-04.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-04/02-precedence.lp",
        &[
            "scenarios/task-allocation/variant-04/02-precedence.lp",
            "encodings/task-allocation/variant-04.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-04/03-agent-serialization.lp",
        &[
            "scenarios/task-allocation/variant-04/03-agent-serialization.lp",
            "encodings/task-allocation/variant-04.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp",
        &[
            "scenarios/task-allocation/variant-04/04-window-too-tight-unsat.lp",
            "encodings/task-allocation/variant-04.lp",
        ],
    ),
    (
        "scenarios/task-allocation/variant-04/05-larger-mix.lp",
        &[
            "scenarios/task-allocation/variant-04/05-larger-mix.lp",
            "encodings/task-allocation/variant-04.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-01/01-basic.lp",
        &[
            "scenarios/traveling-salesman/variant-01/01-basic.lp",
            "encodings/traveling-salesman/variant-01.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-01/02-multiple-tours.lp",
        &[
            "scenarios/traveling-salesman/variant-01/02-multiple-tours.lp",
            "encodings/traveling-salesman/variant-01.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-01/03-asymmetric.lp",
        &[
            "scenarios/traveling-salesman/variant-01/03-asymmetric.lp",
            "encodings/traveling-salesman/variant-01.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp",
        &[
            "scenarios/traveling-salesman/variant-01/04-subtour-unsat.lp",
            "encodings/traveling-salesman/variant-01.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-01/05-ring.lp",
        &[
            "scenarios/traveling-salesman/variant-01/05-ring.lp",
            "encodings/traveling-salesman/variant-01.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-02/01-basic.lp",
        &[
            "scenarios/traveling-salesman/variant-02/01-basic.lp",
            "encodings/traveling-salesman/variant-02.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-02/02-single-salesman.lp",
        &[
            "scenarios/traveling-salesman/variant-02/02-single-salesman.lp",
            "encodings/traveling-salesman/variant-02.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp",
        &[
            "scenarios/traveling-salesman/variant-02/03-too-many-salesmen-unsat.lp",
            "encodings/traveling-salesman/variant-02.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp",
        &[
            "scenarios/traveling-salesman/variant-02/04-equal-cost-split.lp",
            "encodings/traveling-salesman/variant-02.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp",
        &[
            "scenarios/traveling-salesman/variant-02/05-larger-asymmetric.lp",
            "encodings/traveling-salesman/variant-02.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp",
        &[
            "scenarios/traveling-salesman/variant-02/06-unreachable-edge.lp",
            "encodings/traveling-salesman/variant-02.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-03/01-basic.lp",
        &[
            "scenarios/traveling-salesman/variant-03/01-basic.lp",
            "encodings/traveling-salesman/variant-03.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-03/02-single-salesman.lp",
        &[
            "scenarios/traveling-salesman/variant-03/02-single-salesman.lp",
            "encodings/traveling-salesman/variant-03.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp",
        &[
            "scenarios/traveling-salesman/variant-03/03-depot-crossing-unsat.lp",
            "encodings/traveling-salesman/variant-03.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp",
        &[
            "scenarios/traveling-salesman/variant-03/04-equal-cost-split.lp",
            "encodings/traveling-salesman/variant-03.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp",
        &[
            "scenarios/traveling-salesman/variant-03/05-larger-three-depots.lp",
            "encodings/traveling-salesman/variant-03.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp",
        &[
            "scenarios/traveling-salesman/variant-03/06-unreachable-edge.lp",
            "encodings/traveling-salesman/variant-03.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-04/01-basic.lp",
        &[
            "scenarios/traveling-salesman/variant-04/01-basic.lp",
            "encodings/traveling-salesman/variant-04.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-04/02-single-salesman.lp",
        &[
            "scenarios/traveling-salesman/variant-04/02-single-salesman.lp",
            "encodings/traveling-salesman/variant-04.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp",
        &[
            "scenarios/traveling-salesman/variant-04/03-window-too-tight-unsat.lp",
            "encodings/traveling-salesman/variant-04.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp",
        &[
            "scenarios/traveling-salesman/variant-04/04-depot-window-too-tight-unsat.lp",
            "encodings/traveling-salesman/variant-04.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp",
        &[
            "scenarios/traveling-salesman/variant-04/05-three-depots-asymmetric-times.lp",
            "encodings/traveling-salesman/variant-04.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp",
        &[
            "scenarios/traveling-salesman/variant-04/06-unreachable-edge.lp",
            "encodings/traveling-salesman/variant-04.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-05/01-basic.lp",
        &[
            "scenarios/traveling-salesman/variant-05/01-basic.lp",
            "encodings/traveling-salesman/variant-05.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp",
        &[
            "scenarios/traveling-salesman/variant-05/02-tight-bound-exact.lp",
            "encodings/traveling-salesman/variant-05.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp",
        &[
            "scenarios/traveling-salesman/variant-05/03-revisit-too-tight-unsat.lp",
            "encodings/traveling-salesman/variant-05.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp",
        &[
            "scenarios/traveling-salesman/variant-05/04-depot-and-vertex-revisits.lp",
            "encodings/traveling-salesman/variant-05.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp",
        &[
            "scenarios/traveling-salesman/variant-05/05-three-depots-mixed.lp",
            "encodings/traveling-salesman/variant-05.lp",
        ],
    ),
    (
        "scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp",
        &[
            "scenarios/traveling-salesman/variant-05/06-unreachable-edge.lp",
            "encodings/traveling-salesman/variant-05.lp",
        ],
    ),
    (
        "standalone/n-queens/variant-01.lp",
        &["standalone/n-queens/variant-01.lp"],
    ),
    (
        "standalone/n-queens/variant-02.lp",
        &["standalone/n-queens/variant-02.lp"],
    ),
    (
        "standalone/n-queens/variant-03.lp",
        &["standalone/n-queens/variant-03.lp"],
    ),
    (
        "standalone/n-queens/variant-04.lp",
        &["standalone/n-queens/variant-04.lp"],
    ),
    (
        "standalone/n-queens/variant-05.lp",
        &["standalone/n-queens/variant-05.lp"],
    ),
    (
        "standalone/n-queens/variant-06.lp",
        &["standalone/n-queens/variant-06.lp"],
    ),
    (
        "standalone/send-money/send-money.lp",
        &["standalone/send-money/send-money.lp"],
    ),
];

#[test]
fn correctness_entry_graphs_load_with_original_source_identity() {
    use std::collections::BTreeSet;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/correctness")
        .canonicalize()
        .expect("correctness examples root");
    assert_eq!(CORPUS_CASES.len(), 94);
    let mut all_sources = BTreeSet::new();
    for &(entry, expected_paths) in CORPUS_CASES {
        let bundle = SourceBundle::load(root.join(entry), BundleLimits::default())
            .unwrap_or_else(|error| panic!("native graph {entry}: {error}"));
        let expected: BTreeSet<_> = expected_paths.iter().map(|path| root.join(path)).collect();
        let actual: BTreeSet<_> = bundle
            .sources()
            .iter()
            .map(|source| source.path().to_path_buf())
            .collect();
        assert_eq!(actual, expected, "include closure for {entry}");
        assert_eq!(
            bundle.get(bundle.entry()).expect("entry source").path(),
            root.join(entry)
        );
        let ids: Vec<_> = bundle.sources().iter().map(BundleSource::id).collect();
        assert!(
            check_sources_laws(&bundle, &ids).is_empty(),
            "source laws for {entry}"
        );
        for source in bundle.sources() {
            assert_eq!(source.parsed().source(), source.id());
            assert!(
                source.parsed().diagnostics().is_empty(),
                "clean parse for {}",
                source.path().display()
            );
            assert_eq!(
                source.source().text().as_bytes(),
                fs::read(source.path()).expect("original source bytes")
            );
            for edge in source.includes() {
                assert_eq!(edge.location().source, source.id());
                assert!(bundle.get(edge.target()).is_some(), "resolved target");
            }
        }
        all_sources.extend(actual);
    }
    assert_eq!(all_sources.len(), 108, "all correctness sources loaded");
}
