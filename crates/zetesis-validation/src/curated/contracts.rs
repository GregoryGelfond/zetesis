//! Integrity of identities and full-model versus selected-display contracts.

use super::document::{CaseRecord, Document};
use super::{Error, Limits, Resource, UPSTREAM_REVISION, assertion, ceiling, files};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const ORIGINALS: &[(&str, &str)] = &[
    (
        "libgringo/tests/output/lparse.cc",
        "f1071f4e5a4a18c818cff85ded75ea7b048ae1303d8271feb099d19bd005e87b",
    ),
    (
        "libgringo/tests/output/aspcomp13.cc",
        "dddbc1befc29b251865b1c359c464c2f441fb5de499f07f90d1ed792cbdcd8a0",
    ),
    (
        "libgringo/tests/output/solver_helper.hh",
        "c657770081a59aba05f4f96ac308e6b5544d5c76f4ae6809d970e0136d1ae373",
    ),
];
pub(super) fn validate(document: &Document, limits: Limits) -> Result<(), Error> {
    let origins: BTreeMap<_, _> = document
        .origins
        .iter()
        .map(|origin| (origin.path.as_str(), origin))
        .collect();
    if origins.len() != document.origins.len() || origins.len() != ORIGINALS.len() {
        return Err(Error::Contract(
            "duplicate or missing original-file authority".into(),
        ));
    }
    for &(path, hash) in ORIGINALS {
        let origin = origins
            .get(path)
            .ok_or_else(|| Error::Contract(format!("missing original: {path}")))?;
        let url = format!("https://github.com/potassco/clingo/blob/{UPSTREAM_REVISION}/{path}");
        if origin.sha256 != hash
            || origin.url != url
            || !origin.copyright_notice.starts_with("// {{{ MIT License\n")
            || !origin
                .copyright_notice
                .contains("// Copyright 2017 Roland Kaminski\n")
        {
            return Err(Error::Contract(format!(
                "changed original authority: {path}"
            )));
        }
    }
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut models = 0_u128;
    let mut atoms = 0_u128;
    for case in &document.cases {
        if !ids.insert(&case.id) || !paths.insert(&case.path) {
            return Err(Error::Contract(format!(
                "duplicate case identity or path: {}",
                case.id
            )));
        }
        identity(case, &origins)?;
        models += case.contract.full_models.len() as u128;
        ceiling(Resource::Models, models, limits.models)?;
        for model in &case.contract.full_models {
            atoms += model.len() as u128;
            ceiling(Resource::Atoms, atoms, limits.atoms)?;
            let unique: BTreeSet<_> = model.iter().collect();
            if unique.len() != model.len() || model.iter().any(String::is_empty) {
                return Err(Error::Contract(format!(
                    "{}: full model has empty or repeated atoms",
                    case.id
                )));
            }
        }
        let mut selected: Vec<Vec<String>> = case
            .contract
            .full_models
            .iter()
            .map(|model| {
                let mut selected: Vec<_> = model
                    .iter()
                    .filter(|atom| {
                        case.contract
                            .prefixes
                            .iter()
                            .any(|prefix| atom.starts_with(prefix))
                    })
                    .cloned()
                    .collect();
                selected.sort();
                selected
            })
            .collect();
        selected.sort();
        if selected != case.contract.helper_models {
            return Err(Error::Contract(format!(
                "{}: helper projection changes model multiplicity or atoms",
                case.id
            )));
        }
    }
    Ok(())
}
fn identity(
    case: &CaseRecord,
    origins: &BTreeMap<&str, &super::document::OriginRecord>,
) -> Result<(), Error> {
    let origin = &case.provenance;
    files::relative(&case.id)?;
    files::relative(&case.path)?;
    let file = files::relative(&origin.source_file)?;
    let stem = file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| Error::Contract("original filename is not UTF-8".into()))?;
    let identity = format!("{stem}/{}/{:02}", origin.section, origin.assertion_ordinal);
    let lines = origin
        .assertion
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count();
    if case.id != identity
        || case.path != format!("programs/{}.lp", case.id)
        || case.license != "MIT"
        || !origins.contains_key(origin.source_file.as_str())
        || origin.assertion_ordinal == 0
        || origin.section.is_empty()
        || origin.byte_end.checked_sub(origin.byte_start) != Some(origin.assertion.len())
        || origin.line_start == 0
        || origin.line_end.checked_sub(origin.line_start) != Some(lines)
        || origin.helper_arguments.len() > 1
    {
        return Err(Error::Contract(format!(
            "{}: inconsistent source provenance",
            case.id
        )));
    }
    if case.source_sha256.len() != 64
        || !case
            .source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(Error::Contract(format!(
            "{}: invalid source digest",
            case.id
        )));
    }
    Ok(())
}

/// Reconcile the preserved assertion with separately stored source and contracts.
/// Full-model expectations stay independent of the original helper's projection.
pub(super) fn source(case: &CaseRecord, source: &str) -> Result<(), Error> {
    let provenance = &case.provenance;
    let (decoded, arguments, expected) = assertion::assertion(&provenance.assertion)?;
    if decoded != source
        || arguments != provenance.helper_arguments
        || expected != provenance.expected_helper_output
        || assertion::prefixes(&arguments)? != case.contract.prefixes
        || assertion::helper_models(&expected)? != case.contract.helper_models
    {
        return Err(Error::Contract(format!(
            "{}: assertion differs from decoded source or helper contract",
            case.id
        )));
    }
    Ok(())
}
