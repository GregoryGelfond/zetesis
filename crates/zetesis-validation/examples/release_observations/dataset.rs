//! Fixed comparison identities and embedded evidence, separate from table logic.

/// One explicitly selected comparison's expected identities and observation view.
/// These borrows do not admit arbitrary files or launch measurement processes.
#[derive(Clone, Copy)]
pub(super) struct Dataset<'a> {
    pub sources: [&'a str; 2],
    pub binaries: [&'a str; 2],
    pub joins: [Option<&'a str>; 2],
    pub observations: &'a str,
    pub provenance: &'a str,
    pub tables: &'a str,
}

pub(super) const HISTORICAL: Dataset<'static> = Dataset {
    sources: [
        "6bebb980f9c102dbb7f943076d7cde92374841ce",
        "1e5b78ce913ab3aeece6ed496f69ca8176f0644d",
    ],
    binaries: [
        "35b96c837dd5027853c735044e092f9054d63fc516940ec83d10627ecc2cf5d8",
        "0758210934e1a80c350808fc936414c359974a2327119e0af3b3ab5e5f0f79f8",
    ],
    joins: [None, Some("indexed")],
    observations: include_str!(
        "../../../../docs/book/reference/observations/release-6bebb980-1e5b78ce.json"
    ),
    provenance: include_str!(
        "../../../../docs/book/reference/observations/release-6bebb980-1e5b78ce-provenance.json"
    ),
    tables: include_str!(
        "../../../../docs/book/reference/observations/release-6bebb980-1e5b78ce-tables.md"
    ),
};
