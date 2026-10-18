use super::*;
use crate::FormulaResource;
use crate::expansion::Budget;
use crate::formula_ground::Purpose;
use crate::test_support::location;

fn inspect(
    edges: &[[usize; 2]],
    allowance: Option<u64>,
    cancelled: bool,
) -> (Result<Vec<Vec<usize>>, FormulaFailure>, u64) {
    testing::Fixture::default().with(location(), |_, computation, counters| {
        let ordinary = FormulaLimits::default();
        let mut bounded = FormulaLimits::default();
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut builder = Builder::empty(
            computation,
            &ordinary,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            location(),
        )
        .unwrap();
        let rule = RuleIr {
            head: HeadIr::Normal(None),
            body: Vec::new(),
            body_variables: 0,
            bindings: None,
            variables: 0,
            origins: vec![location()],
            location: location(),
        };
        let mut pairs =
            Buffer::new(builder.computation, &ordinary, builder.counters, location()).unwrap();
        for &edge in edges {
            pairs
                .push(
                    edge,
                    builder.computation,
                    &ordinary,
                    builder.counters,
                    location(),
                )
                .unwrap();
        }
        let before = builder.counters.accounting.work;
        let retained = builder.counters.workspace_bytes();
        bounded.max_work = allowance.map_or(u64::MAX, |allowed| before + allowed);
        builder.limits = &bounded;
        let cancellation = Cancellation::default();
        if cancelled {
            builder.counters.attach_test_control(&cancellation);
            cancellation.cancel();
        }
        let result = (|| {
            let plan = plan::Plan::new(&mut pairs, &mut builder, &rule)?;
            let members = plan.members(&mut builder, &rule)?;
            let mut groups = Vec::new();
            let mut start = 0;
            while start < members.len() {
                let component = members.slice()[start][0];
                let mut end = start + 1;
                while end < members.len() && members.slice()[end][0] == component {
                    end += 1;
                }
                if plan.complete(component) {
                    groups.push(
                        members.slice()[start..end]
                            .iter()
                            .map(|entry| entry[1])
                            .collect(),
                    );
                }
                start = end;
            }
            groups.sort();
            Ok(groups)
        })();
        builder.counters.detach_test_control();
        assert_eq!(
            builder.counters.workspace_bytes(),
            retained,
            "private planning leases retire"
        );
        assert!(builder.roots.is_empty(), "planning never publishes a root");
        (result, builder.counters.accounting.work - before)
    })
}

#[test]
fn every_small_graph_certifies_exactly_its_complete_components() {
    let possible = [[0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3]];
    for mask in 0..1_usize << possible.len() {
        let mut edges = Vec::new();
        let mut adjacent = [[false; 4]; 4];
        for (index, &[left, right]) in possible.iter().enumerate() {
            if mask & (1 << index) != 0 {
                // Symmetric duplicates must not impersonate missing pairs.
                edges.extend([[right, left], [left, right], [right, left]]);
                adjacent[left][right] = true;
                adjacent[right][left] = true;
            }
        }
        let mut reachable = adjacent;
        for middle in 0..4 {
            for left in 0..4 {
                for right in 0..4 {
                    reachable[left][right] |= reachable[left][middle] && reachable[middle][right];
                }
            }
        }
        let mut expected = Vec::new();
        for (first, row) in reachable.iter().enumerate() {
            if row[..first].iter().any(|&reached| reached) {
                continue;
            }
            let group: Vec<_> = (0..4).filter(|&node| node == first || row[node]).collect();
            if group.len() >= 3
                && group.iter().all(|&left| {
                    group
                        .iter()
                        .all(|&right| left == right || adjacent[left][right])
                })
            {
                expected.push(group);
            }
        }
        expected.sort();
        assert_eq!(
            inspect(&edges, None, false).0.unwrap(),
            expected,
            "mask={mask}"
        );
    }
}

#[test]
fn every_stopped_planning_prefix_refuses_publication() {
    let edges = [[3, 1], [2, 3], [1, 2], [1, 3], [2, 1], [3, 2]];
    let (complete, work) = inspect(&edges, None, false);
    assert_eq!(complete.unwrap(), vec![vec![1, 2, 3]]);
    assert!(
        work > edges.len() as u64,
        "sorting, union and grouping all run"
    );
    for allowed in 0..work {
        let (result, spent) = inspect(&edges, Some(allowed), false);
        assert!(
            matches!(result, Err(FormulaFailure::Limit {
            resource: FormulaResource::Work, location: origin, ..
        }) if origin == location()),
            "allowed={allowed}"
        );
        assert_eq!(spent, allowed);
    }
}

#[test]
fn cancelled_planning_refuses_publication() {
    let (result, spent) = inspect(&[[1, 2], [1, 3], [2, 3]], None, true);
    assert!(matches!(result, Err(FormulaFailure::Interrupted {
        reason: zetesis_cpu::Stop::Cancelled, location: origin,
    }) if origin == location()));
    assert_eq!(spent, 0);
}
