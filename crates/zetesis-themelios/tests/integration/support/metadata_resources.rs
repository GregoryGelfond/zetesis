//! The metadata limit each metadata resource is counted against, set by
//! resource: metadata compilation is not an admission, so its pairing lives
//! with the only tests that set it rather than in the shared resource view.

use zetesis_themelios::{MetadataLimits, MetadataResource};

/// The default metadata limits, with `resource` bounded by `limit`.
pub(crate) fn limits(resource: MetadataResource, limit: usize) -> MetadataLimits {
    let mut limits = MetadataLimits::default();
    *match resource {
        MetadataResource::Statements => &mut limits.max_statements,
        MetadataResource::Nodes => &mut limits.max_nodes,
        MetadataResource::Depth => &mut limits.max_depth,
        MetadataResource::TextBytes => &mut limits.max_text_bytes,
        MetadataResource::Origins => &mut limits.max_origins,
    } = limit;
    limits
}
