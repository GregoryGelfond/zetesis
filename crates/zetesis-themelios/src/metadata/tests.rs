//! Published identity remains semantic while all products share one vocabulary.
use super::*;
use themelios_base::source::{Source, SourceId};
use themelios_program::raise::raise;
use themelios_syntax::{dialect::Dialect, parse::parse};

fn source(text: &str) -> (SourceProgram, Location) {
    let source = Source::new(SourceId::new(11), text.into()).unwrap();
    let location = Location {
        source: source.id(),
        span: source.span(),
    };
    let parsed = parse(&source, Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let raised = raise(&parsed);
    assert!(raised.diagnostics().is_empty());
    (raised.program().clone(), location)
}

#[test]
fn directive_and_selector_names_borrow_the_same_payload() {
    let (program, location) = source("#defined shared/0. #show shared/0. #project shared/0.");
    let metadata = SourceMetadata::compile(&program, MetadataLimits::default(), location).unwrap();
    let SourceDirective::Defined(defined) = metadata.directives().at(0).unwrap().directive() else {
        panic!("defined occurrence")
    };
    let output = metadata.output().signatures().at(0).unwrap();
    let projection = metadata.project_selection().signatures().at(0).unwrap();
    assert_eq!(defined, output);
    assert_eq!(defined.name().as_ptr(), output.name().as_ptr());
    assert_eq!(defined.name().as_ptr(), projection.name().as_ptr());
}

#[test]
fn foreign_metadata_owners_compare_content_independently_of_admission_order() {
    let (program, location) = source("#show shown(f(1),\"text\").");
    let mut left = Builder::default();
    let extra = OwnedPredicate::new("earlier", 1).unwrap();
    left.admission()
        .unwrap()
        .predicate((&extra).into(), 0)
        .unwrap();
    let mut right = Builder::default();
    for builder in [&mut left, &mut right] {
        collect_profile(&program, builder, true).unwrap();
        builder
            .compile_observations(
                &program,
                crate::AdmissionOptions::default().into(),
                crate::observation::AdmissionLimits::default(),
                &mut crate::expansion::Budget::new(crate::ExpansionLimits::default(), 0),
                location.into(),
            )
            .unwrap();
    }
    let left = left.finish(location.into()).unwrap();
    let right = right.finish(location.into()).unwrap();
    assert_eq!(left, right);
    let (different, location) = source("#show shown(f(2),\"text\").");
    let different =
        SourceMetadata::compile(&different, MetadataLimits::default(), location).unwrap();
    assert_ne!(left.observations(), different.observations());
}

#[test]
fn signature_order_is_semantic_across_input_permutations() {
    let a = OwnedPredicate::new("a", 1).unwrap();
    let z = OwnedPredicate::new("z", 0).unwrap();
    let left = OutputSelection::from_signatures(
        &[z.clone(), a.clone(), a.clone()],
        AtomSelectionLimits::default(),
    )
    .unwrap();
    let right = OutputSelection::from_signatures(&[a, z], AtomSelectionLimits::default()).unwrap();
    assert_eq!(left, right);
    assert_eq!(
        left.signatures()
            .iter()
            .map(zetesis_core::catalog::PredicateRef::name)
            .collect::<Vec<_>>(),
        ["a", "z"]
    );
}

#[test]
fn metadata_storage_refusal_keeps_its_typed_cause_and_location() {
    let (program, location) = source("#show p/0.");
    let limits = MetadataLimits {
        storage: MetadataStorageLimits { max_bytes: 0 },
        ..MetadataLimits::default()
    };
    let error = SourceMetadata::compile(&program, limits, location).unwrap_err();
    let MetadataError::Compilation(crate::FormulaFailure::Expansion(
        crate::ExpansionFailure::Admission(AdmissionFailure::Metadata {
            error,
            location: actual,
        }),
    )) = error
    else {
        panic!("typed located metadata refusal: {error:?}")
    };
    assert_eq!(actual.location().unwrap().source, location.source);
    let MetadataStorageError::Components(zetesis_core::TemplateCatalogFailure::Storage(
        zetesis_core::catalog::Error::Storage { required, limit },
    )) = error
    else {
        panic!("storage ceiling cause: {error:?}")
    };
    assert_eq!(limit, 0);
    assert!(required > 0);
}

#[test]
fn common_collector_applies_constructed_policy_without_source_evidence() {
    let (parsed, location) = source("#defined p/1. #show. #show p/1. #project p/1.");
    let expected = SourceMetadata::compile(&parsed, MetadataLimits::default(), location).unwrap();
    let constructed = SourceProgram::of(parsed.statements().map(|carrier| carrier.get().clone()));
    let mut builder = Builder::default();
    collect_profile(&constructed, &mut builder, true).unwrap();
    let actual = builder.finish(ProgramSite::program()).unwrap();
    assert!(actual.directives().is_empty());
    assert_eq!(actual.atom_selection(), expected.atom_selection());
    assert_eq!(actual.project_selection(), expected.project_selection());
}
