//! A refused inherited-state copy keeps coverage and the attempted receipt.

use super::*;

#[derive(Debug, PartialEq, Eq)]
struct State {
    path: Vec<bool>,
}

#[derive(Default)]
struct Context {
    visits: usize,
    copies: usize,
    refuse_copy: bool,
}

fn narrow(context: &mut Context, _: &mut Entry<State>) -> Narrowing {
    context.visits += 1;
    Narrowing::Fixed { changed: false }
}

fn copy(context: &mut Context, source: &State) -> Result<State, Failure> {
    context.copies += 1;
    if context.refuse_copy {
        Err(Failure::Narrowing(97))
    } else {
        Ok(State {
            path: source.path.clone(),
        })
    }
}

#[test]
fn a_refused_copy_preserves_the_attempted_split() {
    let mut context = Context {
        refuse_copy: true,
        ..Context::default()
    };
    let mut walk = Walk::with_state(
        Region::all_open(3),
        Counting::Never,
        State { path: vec![true] },
    );
    let result = walk.next_with_copy(
        &mut context,
        |context, entry| {
            context.visits += 1;
            assert!(entry.region.hold(2));
            entry.prefer(0);
            entry.state.path.push(false);
            Ok(Narrowing::Fixed { changed: true })
        },
        copy,
    );
    assert_eq!(result, Err(Failure::Narrowing(97)));
    assert_eq!(context.visits, 1);
    assert_eq!(context.copies, 1);
    assert!(walk.root);
    assert!(!walk.narrowed_root);
    assert_eq!(walk.statistics.regions, 1);
    assert_eq!(walk.statistics.splits_since(RegionStatistics::default()), 1);
    let pending = walk.surrender();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].state.path, vec![true, false]);
    assert_eq!(pending[0].split_atom(), Some(0));
    assert_eq!(represented(&pending[0].region), vec![4, 5, 6, 7]);
}

#[test]
fn a_refused_pre_narrowed_copy_keeps_root_policy() {
    for held_first in [false, true] {
        let mut context = Context {
            refuse_copy: true,
            ..Context::default()
        };
        let mut walk = Walk::with_narrowed_root(
            Entry::new(Region::all_open(2), State { path: Vec::new() }),
            Counting::Unchanged,
        );
        walk.held_first = held_first;
        assert_eq!(
            walk.next_with_copy(
                &mut context,
                |context, entry| Ok(narrow(context, entry)),
                copy
            ),
            Err(Failure::Narrowing(97))
        );
        assert_eq!(context.visits, 0);
        assert!(walk.root && walk.narrowed_root);
        context.refuse_copy = false;
        let mut values = Vec::new();
        while let Some(visit) = walk
            .next_with_copy(
                &mut context,
                |context, entry| Ok(narrow(context, entry)),
                copy,
            )
            .unwrap()
        {
            let Visit::Counted(region, _) = visit else {
                panic!("an unchanged child is counted")
            };
            values.push(region.decision(1));
        }
        assert_eq!(values, vec![Some(held_first), Some(!held_first)]);
        assert_eq!(context.visits, 2);
        assert_eq!(context.copies, 2);
        assert_eq!(walk.statistics.regions, 4);
        assert_eq!(walk.statistics.counted, 2);
    }
}

#[test]
fn copying_without_clone_retains_complete_coverage() {
    for held_first in [false, true] {
        let mut context = Context::default();
        let mut walk = Walk::with_state(
            Region::all_open(3),
            Counting::Never,
            State { path: Vec::new() },
        );
        walk.held_first = held_first;
        let mut values = Vec::new();
        while let Some(visit) = walk
            .next_with_copy(
                &mut context,
                |context, entry| Ok(narrow(context, entry)),
                copy,
            )
            .unwrap()
        {
            let Visit::Leaf(region, _) = visit else {
                panic!("every visited candidate is a leaf")
            };
            values.extend(represented(&region));
        }
        let mut expected = (0..8).collect::<Vec<_>>();
        if held_first {
            expected.reverse();
        }
        assert_eq!(values, expected);
        assert_eq!(context.copies, 7);
        assert_eq!(context.visits, 15);
    }
}

#[test]
fn siblings_own_independent_inherited_state() {
    let mut context = Context::default();
    let mut walk = Walk::with_state(
        Region::all_open(1),
        Counting::Never,
        State { path: vec![true] },
    );
    let mut seen = Vec::new();
    while let Some(Visit::Leaf(region, state)) = walk
        .next_with_copy(
            &mut context,
            |_, entry| {
                if let Some(value) = entry.region.decision(0) {
                    entry.state.path.push(value);
                }
                Ok::<_, Failure>(Narrowing::Fixed { changed: false })
            },
            copy,
        )
        .unwrap()
    {
        seen.push((region.decision(0), state.path));
    }
    assert_eq!(
        seen,
        vec![
            (Some(false), vec![true, false]),
            (Some(true), vec![true, true])
        ]
    );
}

#[test]
fn a_terminal_entry_needs_no_state_copy() {
    for narrowing in [Narrowing::Refuted, Narrowing::Fixed { changed: false }] {
        let mut walk = Walk::with_state(
            Region::all_open(0),
            Counting::Never,
            State { path: Vec::new() },
        );
        let result = walk
            .next_with_copy(
                &mut (),
                |(), _| Ok::<_, Failure>(narrowing),
                |(), _| panic!("a terminal entry has no sibling"),
            )
            .unwrap();
        assert_eq!(result.is_none(), narrowing == Narrowing::Refuted);
        assert_eq!(walk.pending(), 0);
    }
}

#[test]
fn reservation_refusal_precedes_state_copy() {
    let mut walk = Walk::with_state(
        Region::all_open(1),
        Counting::Never,
        State { path: Vec::new() },
    );
    let result = walk.next_with_split(
        &mut Context::default(),
        |context, entry| Ok(narrow(context, entry)),
        |_, pending, additional| refuse(pending, additional),
        |_, _, _, _| panic!("refused capacity cannot copy or split"),
    );
    assert_eq!(result, Err(Failure::Stopped(Stop::Allocation)));
    assert_eq!(walk.pending(), 1);
}

#[test]
fn a_narrowing_refusal_precedes_state_copy() {
    let mut walk = Walk::with_state(
        Region::all_open(1),
        Counting::Never,
        State { path: Vec::new() },
    );
    let result = walk.next_with_copy(
        &mut (),
        |(), _| Err(Failure::Narrowing(23)),
        |(), _| panic!("a stopped narrowing cannot copy lineage"),
    );
    assert_eq!(result, Err(Failure::Narrowing(23)));
    assert_eq!(walk.statistics(), RegionStatistics::default());
    assert_eq!(walk.pending(), 1);
}
