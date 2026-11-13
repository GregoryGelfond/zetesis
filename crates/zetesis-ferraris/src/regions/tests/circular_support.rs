//! The support cut passes over circular producers, and the positive-cycle
//! test leaves their dependency out.

use crate::test_support::theory;
use zetesis_algebra::{Cancellation, Stop};

use crate::{
    Narrower, Narrowing, NarrowingScratch, Node, OriginalSubject, Producers, Region, RegionLimits,
    Theory,
};

/// Free choices `e` and `f` (atoms 0 and 1) and a pair `a`, `b` (atoms 2 and
/// 3) joined by `b ← a` and `a ← b`. `a ← e` founds the pair, and `b ← f`
/// founds it a second time when `both` is set.
fn pair(both: bool) -> Theory {
    let mut nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::atom(3),
        Node::falsum(),
        Node::implies(0, 4),
        Node::or_pair([0, 5]),
        Node::implies(1, 4),
        Node::or_pair([1, 7]),
        Node::implies(0, 2),
        Node::implies(2, 3),
        Node::implies(3, 2),
    ];
    let mut roots = vec![6, 8, 9, 10, 11];
    if both {
        nodes.push(Node::implies(1, 3));
        roots.push(nodes.len() - 1);
    }
    theory(4, nodes, roots)
}

fn producers(theory: &Theory) -> Producers {
    super::super::producers(theory, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .expect("every root is a rule of the support fragment")
}

/// Narrow the region holding and cutting the given atoms; return the verdict,
/// the narrowed region and whether the theory kept a positive cycle.
fn narrowed(theory: &Theory, held: &[usize], cut: &[usize]) -> (Narrowing, Region, Option<bool>) {
    let producers = producers(theory);
    let narrower = Narrower::new(theory);
    let mut region = Region::all_open(theory.atom_count());
    for &atom in held {
        region.hold(atom);
    }
    for &atom in cut {
        region.cut(atom);
    }
    let attempt = narrower.narrow_known(
        OriginalSubject::new(theory, Some(&producers)),
        &mut region,
        &mut narrower.knowledge(),
        &mut NarrowingScratch::default(),
        RegionLimits::default(),
        &Cancellation::default(),
    );
    let cyclic = producers.founded_cycles.get().copied();
    (attempt.result.unwrap(), region, cyclic)
}

#[test]
fn a_held_alias_forces_the_source_of_its_pair() {
    // `b` needs `a`, and the only producer of `a` outside the pair is `a ← e`.
    let (narrowing, region, _) = narrowed(&pair(false), &[3], &[]);
    assert_eq!(narrowing, Narrowing::Fixed { changed: true });
    assert!(region.is_held(2) && region.is_held(0));
    assert!(!region.is_held(1) && !region.is_cut(1));
}

#[test]
fn an_alias_pair_falls_with_its_source() {
    let (narrowing, region, _) = narrowed(&pair(false), &[], &[0]);
    assert_eq!(narrowing, Narrowing::Fixed { changed: true });
    assert!(region.is_cut(2) && region.is_cut(3));
}

#[test]
fn a_held_alias_without_its_source_refutes_the_region() {
    let (narrowing, _, _) = narrowed(&pair(false), &[3], &[0]);
    assert_eq!(narrowing, Narrowing::Refuted);
}

#[test]
fn an_alias_pair_is_no_positive_cycle() {
    let (_, _, cyclic) = narrowed(&pair(false), &[], &[0]);
    assert_eq!(cyclic, Some(false));
}

#[test]
fn a_pair_founded_twice_keeps_both_supports() {
    // Neither atom is derived only through the other, so holding `b` holds
    // `a` by `a ← b` and forces neither source: `b ← f` may support `b`, and
    // `a ← b` may support `a`.
    let (narrowing, region, _) = narrowed(&pair(true), &[3], &[]);
    assert_eq!(narrowing, Narrowing::Fixed { changed: true });
    assert!(region.is_held(2));
    assert!(!region.is_held(0) && !region.is_held(1));
}

#[test]
fn a_pair_founded_twice_keeps_its_positive_cycle() {
    // With both sources cut the pair is unfounded, which only the closure of
    // possible founded support establishes.
    let (narrowing, region, cyclic) = narrowed(&pair(true), &[], &[0, 1]);
    assert_eq!(narrowing, Narrowing::Fixed { changed: true });
    assert!(region.is_cut(2) && region.is_cut(3));
    assert_eq!(cyclic, Some(true));
}

/// Free choices `e` and `f` (atoms 0 and 1) and the rules among `a`, `b` and
/// `c` (atoms 2, 3 and 4) that `selected` marks: alias pairs, a rule requiring
/// its own head, longer loops, and the rules founding them.
fn pool(selected: usize) -> Theory {
    const E: usize = 0;
    const F: usize = 1;
    const A: usize = 2;
    const B: usize = 3;
    const C: usize = 4;
    const POOL: [(&[usize], usize); RULES] = [
        (&[E], A),
        (&[A], B),
        (&[B], A),
        (&[B], C),
        (&[C], B),
        (&[A, E], A),
        (&[F], C),
        (&[A, F], B),
        (&[C], A),
        (&[A, B], C),
        (&[F], B),
        (&[A], C),
    ];
    let mut nodes: Vec<Node> = (0..5).map(Node::atom).collect();
    nodes.push(Node::falsum());
    let falsum = nodes.len() - 1;
    let mut roots = Vec::new();
    for choice in [E, F] {
        nodes.push(Node::implies(choice, falsum));
        nodes.push(Node::or_pair([choice, nodes.len() - 1]));
        roots.push(nodes.len() - 1);
    }
    for (index, &(body, head)) in POOL.iter().enumerate() {
        if selected & (1 << index) == 0 {
            continue;
        }
        let body = match *body {
            [atom] => atom,
            [left, right] => {
                nodes.push(Node::and_pair([left, right]));
                nodes.len() - 1
            }
            _ => unreachable!("a pool body has one or two atoms"),
        };
        nodes.push(Node::implies(body, head));
        roots.push(nodes.len() - 1);
    }
    theory(5, nodes, roots)
}

/// The rules of [`pool`] and of [`shaped`].
const RULES: usize = 12;

/// One more rule `head ← body` among the roots.
fn rule(nodes: &mut Vec<Node>, roots: &mut Vec<usize>, body: usize, head: usize) {
    nodes.push(Node::implies(body, head));
    roots.push(nodes.len() - 1);
}

/// The same atoms with the rules `selected` marks in the shapes [`pool`]
/// lacks: a disjunctive body, a disjunctive head, a choice with a body, one
/// body node shared by two producers and a nested conjunction, among plain
/// rules that close loops through them.
fn shaped(selected: usize) -> Theory {
    const E: usize = 0;
    const F: usize = 1;
    const A: usize = 2;
    const B: usize = 3;
    const C: usize = 4;
    let picked = |index: usize| selected & (1 << index) != 0;
    let mut nodes: Vec<Node> = (0..5).map(Node::atom).collect();
    nodes.push(Node::falsum());
    let falsum = nodes.len() - 1;
    let mut roots = Vec::new();
    for choice in [E, F] {
        nodes.push(Node::implies(choice, falsum));
        nodes.push(Node::or_pair([choice, nodes.len() - 1]));
        roots.push(nodes.len() - 1);
    }
    for (index, (body, head)) in [(E, A), (A, B), (B, A), (C, A), (C, B), (F, B)]
        .into_iter()
        .enumerate()
    {
        if picked(index) {
            rule(&mut nodes, &mut roots, body, head);
        }
    }
    if picked(6) {
        // c ← a ∨ b
        nodes.push(Node::or_pair([A, B]));
        let body = nodes.len() - 1;
        rule(&mut nodes, &mut roots, body, C);
    }
    if picked(7) {
        // a ∨ b ← f
        nodes.push(Node::or_pair([A, B]));
        let head = nodes.len() - 1;
        rule(&mut nodes, &mut roots, F, head);
    }
    if picked(8) {
        // {c} ← b
        nodes.push(Node::implies(C, falsum));
        nodes.push(Node::or_pair([C, nodes.len() - 1]));
        let head = nodes.len() - 1;
        rule(&mut nodes, &mut roots, B, head);
    }
    if picked(9) || picked(10) {
        // a ← b ∧ e and c ← b ∧ e, on one body node
        nodes.push(Node::and_pair([B, E]));
        let body = nodes.len() - 1;
        if picked(9) {
            rule(&mut nodes, &mut roots, body, A);
        }
        if picked(10) {
            rule(&mut nodes, &mut roots, body, C);
        }
    }
    if picked(11) {
        // c ← (a ∧ b) ∧ f
        nodes.push(Node::and_pair([A, B]));
        let inner = nodes.len() - 1;
        nodes.push(Node::and_pair([inner, F]));
        let body = nodes.len() - 1;
        rule(&mut nodes, &mut roots, body, C);
    }
    theory(5, nodes, roots)
}

/// The inventory of `theory` with no producer marked.
fn unmarked(theory: &Theory, limits: RegionLimits) -> Result<super::super::Extraction, Stop> {
    super::super::unmarked_producers(theory, limits, &Cancellation::default())
}

/// Every way to leave open, hold or cut each of the two choices, with at most
/// one of the three derived atoms held.
fn regions() -> Vec<Region> {
    let mut regions = Vec::new();
    for choices in 0..9 {
        for derived in [None, Some(2), Some(3), Some(4)] {
            let mut region = Region::all_open(5);
            for (atom, state) in [(0, choices % 3), (1, choices / 3)] {
                match state {
                    1 => {
                        region.hold(atom);
                    }
                    2 => {
                        region.cut(atom);
                    }
                    _ => {}
                }
            }
            if let Some(atom) = derived {
                region.hold(atom);
            }
            regions.push(region);
        }
    }
    regions
}

/// Close `region` from fresh knowledge on one worker's scratch, which holds
/// what that worker prepared for this theory under this inventory.
fn close(
    (theory, narrower): (&Theory, &Narrower),
    (producers, scratch): (&Producers, &mut NarrowingScratch),
    mut region: Region,
) -> Option<Region> {
    let attempt = narrower.narrow_known(
        OriginalSubject::new(theory, Some(producers)),
        &mut region,
        &mut narrower.knowledge(),
        scratch,
        RegionLimits::default(),
        &Cancellation::default(),
    );
    (attempt.result.unwrap() != Narrowing::Refuted).then_some(region)
}

/// Every region the inventory without marks refutes, the marked one refutes;
/// every atom it cuts or holds, the marked one cuts or holds.
fn never_weaker(pool: fn(usize) -> Theory) {
    for selected in 0..1_usize << RULES {
        let theory = pool(selected);
        let narrower = Narrower::new(&theory);
        let subject = (&theory, &narrower);
        let marked = producers(&theory);
        let unmarked = unmarked(&theory, RegionLimits::default())
            .unwrap()
            .producers
            .unwrap();
        let (mut with_marks, mut without_marks) =
            (NarrowingScratch::default(), NarrowingScratch::default());
        for region in regions() {
            let Some(without) = close(subject, (&unmarked, &mut without_marks), region.clone())
            else {
                assert!(
                    close(subject, (&marked, &mut with_marks), region).is_none(),
                    "rules {selected:#014b}"
                );
                continue;
            };
            let Some(with) = close(subject, (&marked, &mut with_marks), region) else {
                continue;
            };
            for atom in 0..5 {
                assert!(
                    !without.is_cut(atom) || with.is_cut(atom),
                    "rules {selected:#014b}: atom {atom} is cut only without marks"
                );
                assert!(
                    !without.is_held(atom) || with.is_held(atom),
                    "rules {selected:#014b}: atom {atom} is held only without marks"
                );
            }
        }
    }
}

#[test]
fn marks_never_weaken_a_narrowing() {
    never_weaker(pool);
}

#[test]
fn marks_never_weaken_a_narrowing_of_other_rule_shapes() {
    never_weaker(shaped);
}

#[test]
fn each_pool_marks_a_producer_in_an_eighth_of_its_selections() {
    // The comparisons above include marked inventories in at least an eighth
    // of each pool's selections.
    for pool in [pool as fn(usize) -> Theory, shaped] {
        let marked = (0..1_usize << RULES)
            .filter(|&selected| producers(&pool(selected)).any_circular())
            .count();
        assert!(marked >= (1 << RULES) / 8, "marked selections: {marked}");
    }
}

/// Narrow the open root, then decide the two choices one after the other on
/// the same knowledge and the same scratch, and compare with one narrowing of
/// the decided region. Which replay the closure of possible founded support
/// chooses at each step is its own; this states only that the steps reach the
/// fixed point of the whole.
fn stepwise_matches_direct(pool: fn(usize) -> Theory) {
    for selected in 0..1_usize << RULES {
        let theory = pool(selected);
        let producers = producers(&theory);
        let narrower = Narrower::new(&theory);
        for decisions in 0..4_usize {
            let mut region = Region::all_open(5);
            let mut knowledge = narrower.knowledge();
            let mut scratch = NarrowingScratch::default();
            let mut stepwise = Some(());
            for step in [None, Some(0), Some(1)] {
                if let Some(atom) = step {
                    if decisions & (1 << atom) == 0 {
                        region.cut(atom);
                    } else {
                        region.hold(atom);
                    }
                }
                let attempt = narrower.narrow_known(
                    OriginalSubject::new(&theory, Some(&producers)),
                    &mut region,
                    &mut knowledge,
                    &mut scratch,
                    RegionLimits::default(),
                    &Cancellation::default(),
                );
                if attempt.result.unwrap() == Narrowing::Refuted {
                    stepwise = None;
                    break;
                }
            }
            let mut decided = Region::all_open(5);
            for atom in 0..2 {
                if decisions & (1 << atom) == 0 {
                    decided.cut(atom);
                } else {
                    decided.hold(atom);
                }
            }
            let direct = close(
                (&theory, &narrower),
                (&producers, &mut NarrowingScratch::default()),
                decided,
            );
            match (stepwise, direct) {
                (None, None) => {}
                (Some(()), Some(direct)) => {
                    for atom in 0..5 {
                        assert_eq!(
                            (region.is_held(atom), region.is_cut(atom)),
                            (direct.is_held(atom), direct.is_cut(atom)),
                            "rules {selected:#014b}, decisions {decisions:#04b}, atom {atom}"
                        );
                    }
                }
                (stepwise, direct) => panic!(
                    "rules {selected:#014b}, decisions {decisions:#04b}: stepwise refuted is {}, direct refuted is {}",
                    stepwise.is_none(),
                    direct.is_none()
                ),
            }
        }
    }
}

#[test]
fn a_narrowing_in_steps_matches_one_narrowing() {
    stepwise_matches_direct(pool);
}

#[test]
fn a_narrowing_in_steps_matches_one_narrowing_of_other_rule_shapes() {
    stepwise_matches_direct(shaped);
}

/// The full [`pool`] with its extraction's own work and its marking's.
fn measured() -> (Theory, u64, u64) {
    let theory = pool((1 << RULES) - 1);
    let unlimited = RegionLimits { max_work: u64::MAX };
    let plain = unmarked(&theory, unlimited).unwrap().work;
    let marked = super::super::producers(&theory, unlimited, &Cancellation::default()).unwrap();
    // Marks under no ceiling to speak of: the marking completed within its
    // share of the extraction, so only a ceiling's remainder can refuse it.
    assert!(marked.producers.unwrap().any_circular());
    let marking = marked.work - plain;
    assert!(marking > 0);
    (theory, plain, marking)
}

/// The mark of every producer, in the inventory's order.
fn marks(inventory: &Producers) -> Vec<bool> {
    (0..inventory.rules.len())
        .map(|row| inventory.circular(row))
        .collect()
}

#[test]
fn a_marking_beyond_what_the_ceiling_leaves_marks_nothing() {
    // One visit short of the marking: the inventory is complete and unmarked,
    // and the refused marking's visits are reported up to the ceiling.
    let (theory, plain, marking) = measured();
    let ceiling = plain + marking - 1;
    let tight = super::super::producers(
        &theory,
        RegionLimits { max_work: ceiling },
        &Cancellation::default(),
    )
    .unwrap();
    let inventory = tight.producers.expect("the extraction fits its ceiling");
    assert!(!inventory.any_circular());
    assert!(marks(&inventory).iter().all(|&mark| !mark));
    assert_eq!(tight.work, ceiling);
}

#[test]
fn a_marking_within_what_the_ceiling_leaves_marks_as_an_unbounded_one() {
    let (theory, plain, marking) = measured();
    let unlimited = RegionLimits { max_work: u64::MAX };
    let expected = super::super::producers(&theory, unlimited, &Cancellation::default()).unwrap();
    let roomy = super::super::producers(
        &theory,
        RegionLimits {
            max_work: plain + marking,
        },
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(roomy.work, plain + marking);
    assert_eq!(
        marks(&roomy.producers.unwrap()),
        marks(&expected.producers.unwrap())
    );
}

#[test]
fn marking_never_moves_the_stop_of_an_extraction() {
    // Under every ceiling the extraction stops with marks exactly where it
    // stops without them, and its reported work stays within the ceiling, so
    // a caller that charges that work to the account the ceiling came from
    // is never refused.
    let (theory, plain, marking) = measured();
    for ceiling in 0..=plain + marking + 2 {
        let limits = RegionLimits { max_work: ceiling };
        let without = unmarked(&theory, limits);
        let with = super::super::producers(&theory, limits, &Cancellation::default());
        match (without, with) {
            (Err(without), Err(with)) => assert_eq!(without, with, "ceiling {ceiling}"),
            (Ok(without), Ok(with)) => {
                assert!(
                    without.work <= with.work && with.work <= ceiling,
                    "ceiling {ceiling}"
                );
                let any = with
                    .producers
                    .is_some_and(|inventory| inventory.any_circular());
                assert_eq!(any, ceiling >= plain + marking, "ceiling {ceiling}");
            }
            (without, with) => panic!(
                "ceiling {ceiling}: stopped without marks is {}, with marks is {}",
                without.is_err(),
                with.is_err()
            ),
        }
    }
}

/// A free choice `e` (atom 0) and atoms `a_1 … a_k`, each with the rule
/// `a_i ← e` and the rule `a_i ← a_1 ∧ … ∧ a_k`, whose body is one node
/// shared by all `k` rules. Every such rule requires its own head.
fn shared_body(k: usize) -> Theory {
    let mut nodes: Vec<Node> = (0..=k).map(Node::atom).collect();
    nodes.push(Node::falsum());
    let falsum = nodes.len() - 1;
    nodes.push(Node::implies(0, falsum));
    nodes.push(Node::or_pair([0, nodes.len() - 1]));
    let mut roots = vec![nodes.len() - 1];
    nodes.push(Node::and_span(crate::OperandSpan {
        start: 0,
        length: k,
    }));
    let body = nodes.len() - 1;
    for head in 1..=k {
        rule(&mut nodes, &mut roots, body, head);
        rule(&mut nodes, &mut roots, 0, head);
    }
    Theory::new(
        k + 1,
        crate::FormulaParts::new(nodes, (1..=k).collect()).unwrap(),
        roots,
        crate::AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn a_marking_within_its_share_marks_every_rule_of_a_shared_body() {
    let theory = shared_body(8);
    let unlimited = RegionLimits { max_work: u64::MAX };
    let plain = unmarked(&theory, unlimited).unwrap().work;
    let marked = super::super::producers(&theory, unlimited, &Cancellation::default()).unwrap();
    assert!(marked.work < 5 * plain);
    // The choice of `e` comes first; the rules then alternate, the circular
    // `a_i ← a_1 ∧ … ∧ a_k` before `a_i ← e`.
    let expected: Vec<bool> = std::iter::once(false)
        .chain((0..16).map(|row| row % 2 == 0))
        .collect();
    assert_eq!(marks(&marked.producers.unwrap()), expected);
}

#[test]
fn a_marking_beyond_its_share_of_the_extraction_marks_nothing() {
    // The shared body is read once for each of its rules, more than k * k
    // visits where the extraction spends a few for each rule. With no
    // ceiling to speak of, the marking stops at four times the extraction's
    // visits and leaves the inventory unmarked.
    for k in [128, 256] {
        let theory = shared_body(k);
        let unlimited = RegionLimits { max_work: u64::MAX };
        let plain = unmarked(&theory, unlimited).unwrap().work;
        let marked = super::super::producers(&theory, unlimited, &Cancellation::default()).unwrap();
        assert!(!marked.producers.unwrap().any_circular(), "{k} rules");
        assert_eq!(marked.work, 5 * plain, "{k} rules");
    }
}

#[test]
fn a_marking_beyond_its_share_stays_within_every_ceiling() {
    // Where the share and the ceiling's remainder compete, the smaller one
    // bounds the marking: under every ceiling the extraction stops where it
    // does without marks, reports work within the ceiling and marks nothing.
    let theory = shared_body(32);
    let plain = unmarked(&theory, RegionLimits { max_work: u64::MAX })
        .unwrap()
        .work;
    for ceiling in 0..=6 * plain {
        let limits = RegionLimits { max_work: ceiling };
        let with = super::super::producers(&theory, limits, &Cancellation::default());
        if ceiling < plain {
            assert!(matches!(with, Err(Stop::WorkLimit)), "ceiling {ceiling}");
            continue;
        }
        let with = with.unwrap();
        assert_eq!(with.work, ceiling.min(5 * plain), "ceiling {ceiling}");
        assert!(!with.producers.unwrap().any_circular(), "ceiling {ceiling}");
    }
}

/// A free choice `e` (atom 0) and atoms `a_1 … a_n` in a ring. Each has the
/// rule `a_i ← e` and a rule whose body, a node of its own, holds the next
/// `width - 1` atoms of the ring and then `a_i` itself: the rule requires its
/// own head, which the marking meets at the body's last operand.
fn ring(n: usize, width: usize) -> Theory {
    let mut nodes: Vec<Node> = (0..=n).map(Node::atom).collect();
    nodes.push(Node::falsum());
    let falsum = nodes.len() - 1;
    nodes.push(Node::implies(0, falsum));
    nodes.push(Node::or_pair([0, nodes.len() - 1]));
    let mut roots = vec![nodes.len() - 1];
    let mut operands = Vec::new();
    for head in 1..=n {
        let start = operands.len();
        operands.extend((1..width).map(|step| (head - 1 + step) % n + 1));
        operands.push(head);
        nodes.push(Node::and_span(crate::OperandSpan {
            start,
            length: width,
        }));
        let body = nodes.len() - 1;
        rule(&mut nodes, &mut roots, body, head);
        rule(&mut nodes, &mut roots, 0, head);
    }
    Theory::new(
        n + 1,
        crate::FormulaParts::new(nodes, operands).unwrap(),
        roots,
        crate::AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn a_ring_of_seven_atom_bodies_is_marked_within_its_share() {
    let theory = ring(64, 7);
    let unlimited = RegionLimits { max_work: u64::MAX };
    let plain = unmarked(&theory, unlimited).unwrap().work;
    let marked = super::super::producers(&theory, unlimited, &Cancellation::default()).unwrap();
    assert!(marked.work < 5 * plain);
    // The choice of `e` comes first; the rules then alternate, each atom's
    // circular rule before its `a_i ← e`.
    let expected: Vec<bool> = std::iter::once(false)
        .chain((0..128).map(|row| row % 2 == 0))
        .collect();
    assert_eq!(marks(&marked.producers.unwrap()), expected);
}

#[test]
fn a_ring_of_eight_atom_bodies_exhausts_its_share() {
    // Width alone, with no body shared: the marking reads every operand of
    // every body and decides it for the head, where the extraction visited
    // each conjunction once. One more atom in each body than the ring above,
    // and the marking stops at four times the extraction's visits.
    let theory = ring(64, 8);
    let unlimited = RegionLimits { max_work: u64::MAX };
    let plain = unmarked(&theory, unlimited).unwrap().work;
    let marked = super::super::producers(&theory, unlimited, &Cancellation::default()).unwrap();
    assert!(!marked.producers.unwrap().any_circular());
    assert_eq!(marked.work, 5 * plain);
}

#[test]
fn a_requested_stop_ends_the_marking_as_a_stop() {
    // The marking observes control before its first visit and reports the
    // stop; it does not take it for an exhausted allowance and return an
    // unmarked inventory.
    let theory = pool((1 << RULES) - 1);
    let mut inventory = unmarked(&theory, RegionLimits { max_work: u64::MAX })
        .unwrap()
        .producers
        .unwrap();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        inventory.mark(&theory, u64::MAX, &cancellation),
        Err(Stop::Cancelled)
    );
    assert!(!inventory.any_circular());
}
