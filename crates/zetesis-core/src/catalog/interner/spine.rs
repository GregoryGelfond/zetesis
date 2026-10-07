//! Published right spines reuse the existing mutation buffer, never a new index.
//!
//! Every scratch writer first takes its certificate. Only complete atomic
//! publication restores it; refusal or unwind leaves tentative cells untrusted.

pub(super) use crate::ordered_index::spine::{RightSpine, admit, retain};

#[derive(Default)]
pub(super) struct Spines {
    pub semantic: Option<RightSpine>,
    pub discovery: Option<RightSpine>,
}
