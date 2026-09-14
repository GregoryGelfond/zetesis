//! Two overlapping views of one six-block prepared-grounding acquisition.

use super::dataset::Dataset;

pub(super) const PREPARED_GROUNDING: Dataset<'static> = Dataset {
    labels: ["ca10a5e7-1", "679ca856-1", "679ca856-2", "ca10a5e7-2"],
    sources: [
        "ca10a5e7ec84e13fbcc4a23bd0de8b0232c53fe1",
        "679ca8568a6fd8577d9b944fbd99d7c54f666601",
    ],
    binaries: [
        "fd19a078e99c75c1bbaf30e437f5da02aa621fd595554b079bb3bc0b078dae0e",
        "a1d8cd7c640bab9b2a57f2e9dd612ff391c39b77f6dc9be95dbea0f890c13bd2",
    ],
    joins: [Some("indexed"), Some("indexed")],
    observations: include_str!(
        "../../../../docs/book/reference/observations/release-ca10a5e7-679ca856.json"
    ),
    provenance: include_str!(
        "../../../../docs/book/reference/observations/release-ca10a5e7-679ca856-provenance.json"
    ),
    tables: include_str!(
        "../../../../docs/book/reference/observations/release-ca10a5e7-679ca856-tables.md"
    ),
};

pub(super) const PREPARED_ALGORITHMS: Dataset<'static> = Dataset {
    labels: ["f56a5a24-1", "679ca856-1", "679ca856-2", "f56a5a24-2"],
    sources: [
        "f56a5a2496f519d7b71b7c4c8fdc166c355874ff",
        "679ca8568a6fd8577d9b944fbd99d7c54f666601",
    ],
    binaries: [
        "fb4d784313f4b0c9a5078712728e70f17d44c545eae4a6ea67180ec7a49bbb29",
        "a1d8cd7c640bab9b2a57f2e9dd612ff391c39b77f6dc9be95dbea0f890c13bd2",
    ],
    joins: [Some("indexed"), Some("indexed")],
    observations: include_str!(
        "../../../../docs/book/reference/observations/release-f56a5a24-679ca856.json"
    ),
    provenance: include_str!(
        "../../../../docs/book/reference/observations/release-f56a5a24-679ca856-provenance.json"
    ),
    tables: include_str!(
        "../../../../docs/book/reference/observations/release-f56a5a24-679ca856-tables.md"
    ),
};
