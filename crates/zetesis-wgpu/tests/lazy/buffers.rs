//! Ownership controls cover the actual release/fill operations without a GPU.

use std::{cell::RefCell, rc::Rc};

use super::{Buffers, Retention, Slot, release};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    GroupDropped,
    BufferDropped(Slot),
    Allocated(Slot),
}

struct Handle {
    slot: Slot,
    events: Rc<RefCell<Vec<Event>>>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.events
            .borrow_mut()
            .push(Event::BufferDropped(self.slot));
    }
}

struct Group {
    _handles: Vec<Rc<Handle>>,
    events: Rc<RefCell<Vec<Event>>>,
}

impl Drop for Group {
    fn drop(&mut self) {
        self.events.borrow_mut().push(Event::GroupDropped);
    }
}

fn original(events: &Rc<RefCell<Vec<Event>>>) -> (Buffers<Rc<Handle>>, Group) {
    let mut handles = Vec::new();
    let buffers = Buffers::empty().complete(|slot| {
        let handle = Rc::new(Handle {
            slot,
            events: Rc::clone(events),
        });
        handles.push(Rc::clone(&handle));
        handle
    });
    (
        buffers,
        Group {
            _handles: handles,
            events: Rc::clone(events),
        },
    )
}

#[test]
fn discarded_owners_leave_before_replacement_allocation() {
    for result in [false, true] {
        let events = Rc::new(RefCell::new(Vec::new()));
        let (buffers, group) = original(&events);
        let retention = Retention {
            uniform: true,
            inputs: [true, false, true, false],
            result,
        };
        let pending = release(group, buffers, retention);
        let mut expected = vec![
            Event::GroupDropped,
            Event::BufferDropped(Slot::Input(1)),
            Event::BufferDropped(Slot::Input(3)),
        ];
        if !result {
            expected.extend([
                Event::BufferDropped(Slot::Output),
                Event::BufferDropped(Slot::Readback),
            ]);
        }
        assert_eq!(*events.borrow(), expected);
        let complete = pending.complete(|slot| {
            // Any rejected handle or old binding still alive at the first
            // allocation violates the admitted prospective payload bound.
            assert_eq!(&events.borrow()[..expected.len()], expected);
            events.borrow_mut().push(Event::Allocated(slot));
            Rc::new(Handle {
                slot,
                events: Rc::clone(&events),
            })
        });
        assert_eq!(Rc::strong_count(&complete.uniform), 1);
        assert!(
            complete
                .inputs
                .iter()
                .all(|handle| Rc::strong_count(handle) == 1)
        );
        assert_eq!(Rc::strong_count(&complete.output), 1);
        assert_eq!(Rc::strong_count(&complete.readback), 1);
    }
}

#[test]
fn retained_buffers_keep_their_identity() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let (buffers, group) = original(&events);
    let identities = [
        Rc::as_ptr(&buffers.uniform),
        Rc::as_ptr(&buffers.inputs[0]),
        Rc::as_ptr(&buffers.inputs[2]),
        Rc::as_ptr(&buffers.output),
        Rc::as_ptr(&buffers.readback),
    ];
    let complete = release(
        group,
        buffers,
        Retention {
            uniform: true,
            inputs: [true, false, true, false],
            result: true,
        },
    )
    .complete(|slot| {
        Rc::new(Handle {
            slot,
            events: Rc::clone(&events),
        })
    });
    assert_eq!(
        identities,
        [
            Rc::as_ptr(&complete.uniform),
            Rc::as_ptr(&complete.inputs[0]),
            Rc::as_ptr(&complete.inputs[2]),
            Rc::as_ptr(&complete.output),
            Rc::as_ptr(&complete.readback),
        ]
    );
}

#[test]
fn complete_retention_requests_no_buffers() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let (buffers, group) = original(&events);
    let _complete = release(
        group,
        buffers,
        Retention {
            uniform: true,
            inputs: [true; 4],
            result: true,
        },
    )
    .complete(|_| panic!("retained handle was lost"));
    assert_eq!(*events.borrow(), [Event::GroupDropped]);
}

#[test]
fn input_replacement_preserves_each_binding_position() {
    for mask in 0..16 {
        let original = Buffers {
            uniform: 10,
            inputs: [20, 21, 22, 23],
            output: 30,
            readback: 40,
        };
        let pending = original.retain(Retention {
            uniform: true,
            inputs: std::array::from_fn(|index| mask & (1 << index) != 0),
            result: true,
        });
        let mut requested = Vec::new();
        let complete = pending.complete(|slot| {
            let Slot::Input(index) = slot else {
                panic!("only input bindings were released");
            };
            requested.push(index);
            100 + index
        });
        for (index, value) in complete.inputs.into_iter().enumerate() {
            let expected = if mask & (1 << index) == 0 {
                100 + index
            } else {
                20 + index
            };
            assert_eq!(value, expected);
        }
        assert_eq!(
            requested,
            (0..4)
                .filter(|index| mask & (1 << index) == 0)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            (complete.uniform, complete.output, complete.readback),
            (10, 30, 40)
        );
    }
}
