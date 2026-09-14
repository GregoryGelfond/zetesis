//! Fixed CPU observations of the atom-catalog implementation at ca10a5e7.

use super::dataset::Dataset;

pub(super) const ATOM_CATALOG: Dataset<'static> = Dataset {
    labels: ["prior-1", "current-1", "current-2", "prior-2"],
    sources: [
        "1e5b78ce913ab3aeece6ed496f69ca8176f0644d",
        "ca10a5e7ec84e13fbcc4a23bd0de8b0232c53fe1",
    ],
    binaries: [
        "0758210934e1a80c350808fc936414c359974a2327119e0af3b3ab5e5f0f79f8",
        "fd19a078e99c75c1bbaf30e437f5da02aa621fd595554b079bb3bc0b078dae0e",
    ],
    joins: [Some("indexed"), Some("indexed")],
    observations: include_str!(
        "../../../../docs/book/reference/observations/release-1e5b78ce-ca10a5e7.json"
    ),
    provenance: include_str!(
        "../../../../docs/book/reference/observations/release-1e5b78ce-ca10a5e7-provenance.json"
    ),
    tables: include_str!(
        "../../../../docs/book/reference/observations/release-1e5b78ce-ca10a5e7-tables.md"
    ),
};
