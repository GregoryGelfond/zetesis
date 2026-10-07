//! A published right spine in the owner's existing mutation buffer.
//!
//! Every scratch writer first takes its certificate. Only complete atomic
//! publication restores it; refusal or unwind leaves tentative cells untrusted.
//! The owner separately proves that an insertion follows its semantic maximum.

use super::{Index, Link, Planned, Rotation, Step};

#[derive(Clone, Copy)]
pub(crate) struct RightSpine {
    root: Link,
    last: usize,
    length: usize,
}

impl RightSpine {
    /// Root and endpoint come from this same published tree. The owner revokes
    /// this certificate before every mutation/repurposing of its path buffer.
    pub(crate) fn matches(self, index: &Index, root: Link, last: usize) -> bool {
        self.root == root
            && self.last == last
            && self.length == index.path.len()
            && index.path.last().is_some_and(|step| step.id == last)
    }

    pub(crate) fn last(self) -> usize {
        self.last
    }
}

/// All-right insertion permits only a single left rotation. Its old root leaves
/// the right spine, while the promoted child and unchanged prefix remain. Admit
/// the removal's actual shifted suffix and every reset cell before publication.
pub(crate) fn admit<E>(
    index: &Index,
    plan: Planned,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    let removed = match plan.rotation {
        Some(Rotation::Single(at)) => {
            for _ in at + 1..index.path.len() {
                before()?;
            }
            1
        }
        Some(Rotation::Double) => unreachable!("all-right insertion has no double rotation"),
        None => 0,
    };
    for _ in plan.changed_from..index.path.len() - removed {
        before()?;
    }
    before()?; // Replacement certificate publication.
    Ok(())
}

/// Only call after successful publication and `admit`, with an all-right input
/// path. Existing prefix cells already equal published nodes; reset just the
/// planned suffix. No callback, allocation or recoverable failure occurs here.
pub(crate) fn retain(index: &mut Index, plan: Planned, last: usize) -> RightSpine {
    if let Some(rotation) = plan.rotation {
        let Rotation::Single(at) = rotation else {
            unreachable!("admitted right-spine rotation")
        };
        index.path.remove(at);
    }
    for Step { right, changed, .. } in &mut index.path[plan.changed_from..] {
        *right = true;
        *changed = false;
    }
    RightSpine {
        root: plan.root,
        last,
        length: index.path.len(),
    }
}
