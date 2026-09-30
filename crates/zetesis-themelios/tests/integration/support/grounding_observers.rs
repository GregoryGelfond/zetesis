//! An observer that records a grounding's enter and exit events.

use std::cell::RefCell;

use zetesis_themelios::GroundingObserver;

/// Records `true` on each enter and `false` on each exit, in order.
#[derive(Default)]
pub struct Observer(pub RefCell<Vec<bool>>);
impl GroundingObserver for Observer {
    fn enter(&self) {
        self.0.borrow_mut().push(true);
    }
    fn exit(&self) {
        self.0.borrow_mut().push(false);
    }
}
