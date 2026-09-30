//! Adversarial contracts for the private corpus boundary.

use super::*;

#[test]
fn replacement_metadata_requires_a_nonempty_exact_span() {
    for (start, end, before) in [(1, 1, ""), (2, 1, "x"), (0, 2, "x")] {
        assert!(Edit::replacement(start, end, before.into(), "replacement".into()).is_err());
    }
    let edit = Edit::replacement(0, 1, "a".into(), "bc".into()).unwrap();
    assert_eq!(derive_source("a", &[edit], 2).unwrap(), "bc");
}

fn edit() -> Edit {
    Edit {
        start_byte: 2,
        end_byte: 3,
        before: "8".into(),
        after: "n".into(),
    }
}

#[test]
fn edits_preserve_unselected_bytes() {
    assert_eq!(derive_source("p(8).", &[edit()], 5).unwrap(), "p(n).");
}

#[test]
fn edits_keep_original_coordinates_after_growth() {
    let mut first = edit();
    first.after = "100".into();
    let second = Edit {
        start_byte: 4,
        end_byte: 5,
        before: "9".into(),
        after: "1".into(),
    };
    assert_eq!(
        derive_source("p(8,9).", &[first, second], 9).unwrap(),
        "p(100,1)."
    );
}

#[test]
fn edits_require_exact_source_spans() {
    for (start, end, before) in [(2, 2, ""), (0, 1, "8"), (2, 9, "8"), (1, 2, "(")] {
        let mut record = edit();
        record.start_byte = start;
        record.end_byte = end;
        record.before = before.into();
        let source = if before == "(" { "é(8)." } else { "p(8)." };
        assert!(matches!(
            derive_source(source, &[record], 20),
            Err(Error::Contract(_))
        ));
    }
}

#[test]
fn overlapping_edits_are_refused() {
    assert!(matches!(
        derive_source("p(8).", &[edit(), edit()], 20),
        Err(Error::Contract(_))
    ));
}

#[test]
fn edited_sources_obey_the_byte_ceiling() {
    let mut record = edit();
    record.after = "100".into();
    assert_eq!(
        derive_source("p(8).", &[record.clone()], 7).unwrap(),
        "p(100)."
    );
    assert!(matches!(
        derive_source("p(8).", &[record], 6),
        Err(Error::Limit {
            resource: Resource::SourceBytes,
            observed: 7,
            limit: 6
        })
    ));
}

fn annotation(source: &str) -> Annotation {
    Annotation {
        line: 1,
        start_byte: 0,
        end_byte: source.len() + 1,
        source: source.into(),
    }
}

#[test]
fn deletion_preserves_every_noncomment_byte() {
    let source = "% @expect sat\n% description\np(\"@model\").\n";
    assert_eq!(
        remove_annotations(source, &[annotation("% @expect sat")]).unwrap(),
        "% description\np(\"@model\").\n"
    );
}

#[test]
fn partial_comment_deletion_is_refused() {
    let source = "% @expect sat EXTRA\np.\n";
    let mut record = annotation("% @expect sat");
    record.end_byte -= 1;
    assert!(remove_annotations(source, &[record]).is_err());
}

#[test]
fn annotations_inside_block_comments_are_refused() {
    let source = "%* block\n% @expect sat\n*%\np.\n";
    let mut record = annotation("% @expect sat");
    record.line = 2;
    record.start_byte = "%* block\n".len();
    record.end_byte += record.start_byte;
    assert!(remove_annotations(source, &[record]).is_err());
}

#[test]
fn overlapping_deletions_are_refused() {
    let source = "% @expect sat\np.\n";
    assert!(
        remove_annotations(
            source,
            &[annotation("% @expect sat"), annotation("% @expect sat")]
        )
        .is_err()
    );
}

#[test]
fn different_annotation_spelling_is_refused() {
    assert!(remove_annotations("% @expect sat\np.\n", &[annotation("% @expect bad")]).is_err());
}

#[test]
fn conflicting_family_annotations_are_refused() {
    assert!(
        translate(&[
            annotation("% @expect sat"),
            annotation("% @count 1"),
            annotation("% @cost { 2 }")
        ])
        .is_err()
    );
}

#[test]
fn unknown_annotation_tags_are_refused() {
    assert!(translate(&[annotation("% @expect sat"), annotation("% @mystery { p }")]).is_err());
}

#[test]
fn final_comment_without_newline_can_be_removed() {
    let mut record = annotation("% @expect sat");
    record.end_byte -= 1;
    assert_eq!(remove_annotations("% @expect sat", &[record]).unwrap(), "");
}

#[test]
fn invalid_original_contracts_remain_explicit_refusals() {
    for text in [
        "not a comment",
        "% @expect",
        "% @expect unknown",
        "% @count many",
        "% @cost { many }",
        "% @cost 2",
        "% @model { p( }",
        "% @cautious all {p}",
    ] {
        let error = translate(&[annotation("% @expect sat"), annotation(text)]).unwrap_err();
        assert!(matches!(&error, Error::Contract(_)));
        assert!(error.to_string().contains("examples contract"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn absent_original_expectation_is_not_inferred() {
    assert!(translate(&[annotation("% @note no status")]).is_err());
}

#[test]
fn annotation_spans_cannot_split_utf8_characters() {
    let mut record = annotation("% @expect sat");
    record.start_byte = 1;
    record.end_byte = 2;
    assert!(remove_annotations("é\n", &[record]).is_err());
}

#[test]
fn annotation_spans_cannot_extend_past_the_source() {
    let mut record = annotation("% @expect sat");
    record.end_byte = usize::MAX;
    assert!(remove_annotations("% @expect sat\n", &[record]).is_err());
}

#[test]
fn conflicting_satisfiability_annotations_are_refused() {
    assert!(translate(&[annotation("% @expect sat"), annotation("% @expect unsat")]).is_err());
}

#[test]
fn conflicting_count_annotations_are_refused() {
    assert!(
        translate(&[
            annotation("% @expect sat"),
            annotation("% @count 1"),
            annotation("% @count 2"),
        ])
        .is_err()
    );
}

#[test]
fn conflicting_cost_annotations_are_refused() {
    assert!(
        translate(&[
            annotation("% @expect sat"),
            annotation("% @cost { 1 }"),
            annotation("% @cost { 2 }"),
        ])
        .is_err()
    );
}

#[test]
fn witnesses_cannot_mix_selected_families() {
    assert!(
        translate(&[
            annotation("% @expect sat"),
            annotation("% @model { p }"),
            annotation("% @optimal { p }"),
        ])
        .is_err()
    );
}

#[test]
fn required_symbols_cannot_reclassify_ordinary_models() {
    assert!(
        translate(&[
            annotation("% @expect sat"),
            annotation("% @model { p }"),
            annotation("% @cautious optimal { p }"),
        ])
        .is_err()
    );
}

#[test]
fn satisfiable_annotations_cannot_require_no_models() {
    assert!(translate(&[annotation("% @expect sat"), annotation("% @count 0")]).is_err());
}
