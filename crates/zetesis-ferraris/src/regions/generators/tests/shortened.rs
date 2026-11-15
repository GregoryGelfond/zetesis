//! Among heads of one choice depth, the chains a region shortened decide.

use super::{Allowance, extract, prepare, theory};
use crate::regions::tests::shortening;
use crate::regions::{GeneratorOrder, Meter};
use crate::{
    Knowledge, Narrower, Narrowing, NarrowingScratch, Node, OriginalSubject, Region, RegionLimits,
    Theory,
};
use zetesis_algebra::{Cancellation, Stop};

/// A formula under construction over `atoms` atoms: node `a` is atom `a`.
struct Formula {
    atoms: usize,
    nodes: Vec<Node>,
    roots: Vec<usize>,
}

impl Formula {
    fn new(atoms: usize) -> Self {
        let mut nodes: Vec<Node> = (0..atoms).map(Node::atom).collect();
        nodes.push(Node::falsum());
        Self {
            atoms,
            nodes,
            roots: Vec::new(),
        }
    }

    fn push(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// The negation of `node`.
    fn not(&mut self, node: usize) -> usize {
        let falsum = self.atoms;
        self.push(Node::implies(node, falsum))
    }

    /// An atomic choice of `atom`, under `guard` when one is given. Returns
    /// the node of the atom's negation.
    fn choice(&mut self, atom: usize, guard: Option<usize>) -> usize {
        let absent = self.not(atom);
        let choice = self.push(Node::or_pair([atom, absent]));
        let root = match guard {
            Some(guard) => self.push(Node::implies(guard, choice)),
            None => choice,
        };
        self.roots.push(root);
        absent
    }

    /// The conjunction of `operands`, one chain however it is nested.
    fn all(&mut self, operands: &[usize]) -> usize {
        let (&last, rest) = operands.split_last().expect("a conjunction has an operand");
        rest.iter().rev().fold(last, |body, &operand| {
            self.push(Node::and_pair([operand, body]))
        })
    }

    /// The constraint that no answer set satisfies every operand.
    fn never(&mut self, operands: &[usize]) {
        let body = self.all(operands);
        let root = self.not(body);
        self.roots.push(root);
    }

    fn theory(self) -> Theory {
        theory(self.atoms, self.nodes, self.roots)
    }
}

/// `choices` independent choices and one constraint for each list.
fn constrained(choices: usize, constraints: &[&[usize]]) -> Theory {
    let mut formula = Formula::new(choices);
    for atom in 0..choices {
        formula.choice(atom, None);
    }
    for atoms in constraints {
        formula.never(atoms);
    }
    formula.theory()
}

/// A theory with its order and index, and one region closed step by step.
struct Walked {
    theory: Theory,
    order: GeneratorOrder,
    narrower: Narrower,
    region: Region,
    known: Knowledge,
    /// The split the last closure preferred.
    preferred: Option<usize>,
}

impl Walked {
    /// The theory's open region, closed.
    fn open(theory: Theory) -> Self {
        let order = prepare(&theory).result.unwrap().unwrap();
        let narrower = Narrower::new(&theory);
        let mut walked = Self {
            region: Region::all_open(theory.atom_count()),
            known: narrower.knowledge(),
            theory,
            order,
            narrower,
            preferred: None,
        };
        assert!(walked.close(), "the open region is not refuted");
        walked
    }

    /// Hold or cut `atom` and close the region on the knowledge it had, as a
    /// walk's child does. The atom must be open and the region must survive.
    fn decide(self, atom: usize, held: bool) -> Self {
        self.try_decide(atom, held)
            .expect("an open atom in a region the decision does not refute")
    }

    /// [`Self::decide`], or nothing when the atom is already decided or the
    /// decision refutes the region.
    fn try_decide(mut self, atom: usize, held: bool) -> Option<Self> {
        let decided = if held {
            self.region.hold(atom)
        } else {
            self.region.cut(atom)
        };
        (decided && self.close()).then_some(self)
    }

    /// Close the region; false when the closure refutes it.
    fn close(&mut self) -> bool {
        let extraction = extract(&self.theory);
        let attempt = self.narrower.narrow_known(
            OriginalSubject::new(&self.theory, extraction.producers.as_ref())
                .with_generator_order(Some(&self.order)),
            &mut self.region,
            &mut self.known,
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        );
        self.preferred = attempt.preferred;
        attempt.result.unwrap() != Narrowing::Refuted
    }

    /// The score of each open atom; a decided atom has none to rank.
    fn scores(&self) -> Vec<Option<u64>> {
        (0..self.theory.atom_count())
            .map(|atom| {
                self.region.is_open(atom).then(|| {
                    self.narrower
                        .shortened_weight(&self.known, atom, &mut Meter::local(u64::MAX))
                        .map_err(zetesis_algebra::Failure::into_stop)
                        .unwrap()
                })
            })
            .collect()
    }
}

#[test]
fn chains_of_sixteen_or_more_members_weigh_alike() {
    // One constraint of eighteen atoms. With seventeen open it weighs what it
    // did with all eighteen; with fifteen, four times that.
    let atoms: Vec<usize> = (0..18).collect();
    let walked = Walked::open(constrained(18, &[&atoms])).decide(17, true);
    assert!(walked.known.shortened());
    assert!(walked.scores()[..17].iter().all(|&score| score == Some(0)));
    let walked = walked.decide(16, true).decide(15, true);
    assert!(
        walked.scores()[..15]
            .iter()
            .all(|&score| score == Some(shortening(18, 15)))
    );
}

#[test]
fn a_shortened_chain_precedes_an_earlier_component() {
    // No answer set holds 0, 1 and 2 together; 3 is in no constraint and its
    // component comes first.
    let walked = Walked::open(constrained(4, &[&[0, 1, 2]]));
    let ranks = &walked.order.ranks;
    assert!(ranks[3] < ranks[2] && ranks[2] < ranks[1]);
    assert!(!walked.known.shortened());
    assert_eq!(walked.preferred, Some(3));
    let walked = walked.decide(0, true);
    assert!(walked.known.shortened());
    assert_eq!(
        walked.scores(),
        [
            None,
            Some(shortening(3, 2)),
            Some(shortening(3, 2)),
            Some(0)
        ]
    );
    // 1 and 2 score alike, and 2's component is the earlier.
    assert_eq!(walked.preferred, Some(2));
}

#[test]
fn equal_scores_in_one_component_keep_the_lowest_atom() {
    // 0, 1 and 2 derive each other, so they share a component; no answer set
    // holds them with 3.
    let mut formula = Formula::new(4);
    for atom in 0..4 {
        formula.choice(atom, None);
    }
    for (from, to) in [(0, 1), (1, 2), (2, 0)] {
        let rule = formula.push(Node::implies(from, to));
        formula.roots.push(rule);
    }
    formula.never(&[0, 1, 2, 3]);
    let walked = Walked::open(formula.theory()).decide(3, true);
    let ranks = &walked.order.ranks;
    assert!(ranks[0] == ranks[1] && ranks[1] == ranks[2]);
    let shortened = Some(shortening(4, 3));
    assert_eq!(walked.scores(), [shortened, shortened, shortened, None]);
    assert_eq!(walked.preferred, Some(0));
}

#[test]
fn a_chain_as_long_as_written_scores_nothing() {
    // The region shortened the first constraint and left the second whole.
    let walked = Walked::open(constrained(7, &[&[0, 1, 2], &[3, 4, 5]])).decide(0, true);
    let (shortened, whole) = (Some(shortening(3, 2)), Some(0));
    assert_eq!(
        walked.scores(),
        [None, shortened, shortened, whole, whole, whole, whole]
    );
}

#[test]
fn a_known_chain_down_to_one_operand_leaves_no_mark() {
    // The constraint is known to fail, so its last operand is decided with
    // the first: no chain keeps two members.
    let walked = Walked::open(constrained(3, &[&[0, 1]])).decide(0, true);
    assert!(!walked.region.is_open(1));
    assert!(!walked.known.shortened());
    assert_eq!(walked.preferred, Some(2));
}

/// Four choices and the rule `2 :- 0, 1`.
fn body_of_two() -> Theory {
    let mut formula = Formula::new(4);
    for atom in 0..4 {
        formula.choice(atom, None);
    }
    let body = formula.all(&[0, 1]);
    let rule = formula.push(Node::implies(body, 2));
    formula.roots.push(rule);
    formula.theory()
}

#[test]
fn a_body_down_to_one_operand_under_an_open_head_is_shortened() {
    // With 0 held the body leaves two things to decide: 1, and whether the
    // body holds, which decides the head. It weighs as three members down to
    // two do.
    let walked = Walked::open(body_of_two()).decide(0, true);
    assert!(walked.known.shortened());
    assert_eq!(
        walked.scores(),
        [None, Some(shortening(3, 2)), Some(0), Some(0)]
    );
}

#[test]
fn a_region_without_the_mark_scores_nothing() {
    // Five choices under a constraint of two atoms, one of three, and a rule
    // with a body of two: chains with a known truth and with an open one,
    // which a decision can bring to one operand or leave two. Every region
    // reached by at most three decisions is checked.
    let fixture = || {
        let mut formula = Formula::new(5);
        for atom in 0..5 {
            formula.choice(atom, None);
        }
        formula.never(&[0, 1]);
        formula.never(&[1, 2, 3]);
        let body = formula.all(&[2, 3]);
        let rule = formula.push(Node::implies(body, 4));
        formula.roots.push(rule);
        formula.theory()
    };
    let decisions: Vec<(usize, bool)> = (0..5)
        .flat_map(|atom| [(atom, false), (atom, true)])
        .collect();
    let (mut unmarked, mut marked) = (0, 0);
    let mut paths = vec![Vec::new()];
    for _ in 0..3 {
        paths = paths
            .iter()
            .flat_map(|path: &Vec<(usize, bool)>| {
                decisions.iter().map(move |&step| {
                    let mut longer = path.clone();
                    longer.push(step);
                    longer
                })
            })
            .collect();
        for path in &paths {
            let reached = path
                .iter()
                .try_fold(Walked::open(fixture()), |walked, &(atom, held)| {
                    walked.try_decide(atom, held)
                });
            let Some(walked) = reached else { continue };
            if walked.known.shortened() {
                marked += 1;
            } else {
                unmarked += 1;
                assert!(
                    walked.scores().iter().all(|&score| score.unwrap_or(0) == 0),
                    "{path:?}"
                );
            }
        }
    }
    assert!(
        unmarked > 0 && marked > 0,
        "both kinds of region are reached"
    );
}

#[test]
fn descendants_inherit_the_shortened_mark() {
    let walked = Walked::open(constrained(4, &[&[0, 1, 2]])).decide(0, true);
    let mut copy = walked.narrower.knowledge();
    assert!(!copy.shortened());
    copy.clone_from(&walked.known);
    assert!(copy.shortened());
    assert!(walked.known.clone().shortened());
}

#[test]
fn one_chain_down_to_two_outweighs_three_down_to_three() {
    // 0 sits in one constraint of three that lost an operand; 9 sits in three
    // constraints of four that each lost one, and its component is earlier.
    let walked = Walked::open(constrained(
        10,
        &[&[0, 1, 2], &[9, 3, 4, 5], &[9, 3, 4, 6], &[9, 3, 4, 7]],
    ));
    assert!(walked.order.ranks[9] < walked.order.ranks[0]);
    let walked = [2, 5, 6, 7]
        .into_iter()
        .fold(walked, |walked, atom| walked.decide(atom, true));
    let scores = walked.scores();
    assert_eq!(
        (scores[0], scores[1]),
        (Some(shortening(3, 2)), Some(shortening(3, 2)))
    );
    assert_eq!(scores[9], Some(3 * shortening(4, 3)));
    assert!(scores[9] < scores[0]);
    // 1 scores what 0 does, and its component is the earlier of the two.
    assert!(walked.order.ranks[1] < walked.order.ranks[0]);
    assert_eq!(walked.preferred, Some(1));
}

#[test]
fn a_chain_of_unknown_truth_has_its_truth_as_a_member() {
    // 3 :- 0, 1, 2. Until 3 is decided the body may hold or fail, so with 0
    // held it has gone from four members to three.
    let mut formula = Formula::new(4);
    for atom in 0..4 {
        formula.choice(atom, None);
    }
    let body = formula.all(&[0, 1, 2]);
    let rule = formula.push(Node::implies(body, 3));
    formula.roots.push(rule);
    let walked = Walked::open(formula.theory()).decide(0, true);
    assert_eq!(
        walked.scores(),
        [
            None,
            Some(shortening(4, 3)),
            Some(shortening(4, 3)),
            Some(0)
        ]
    );
}

#[test]
fn a_settled_chain_scores_nothing() {
    // 0 | 1 | 2. Cutting 0 shortens the disjunction; holding 1 settles it.
    let mut formula = Formula::new(4);
    for atom in 0..4 {
        formula.choice(atom, None);
    }
    let rest = formula.push(Node::or_pair([1, 2]));
    let fact = formula.push(Node::or_pair([0, rest]));
    formula.roots.push(fact);
    let walked = Walked::open(formula.theory()).decide(0, false);
    assert_eq!(
        walked.scores(),
        [
            None,
            Some(shortening(3, 2)),
            Some(shortening(3, 2)),
            Some(0)
        ]
    );
    let walked = walked.decide(1, true);
    // The mark stays, and permits a scan that finds nothing.
    assert!(walked.known.shortened());
    assert_eq!(walked.scores(), [None, None, Some(0), Some(0)]);
    let ranks = &walked.order.ranks;
    let first = [2, 3]
        .into_iter()
        .min_by_key(|&atom| (ranks[atom], atom))
        .unwrap();
    assert_eq!(walked.preferred, Some(first));
}

#[test]
fn a_negated_occurrence_is_read_through_its_implication() {
    // No answer set omits 0 and holds 1 and 2: the chain holds 0's negation.
    let mut formula = Formula::new(3);
    let absent = formula.choice(0, None);
    formula.choice(1, None);
    formula.choice(2, None);
    formula.never(&[absent, 1, 2]);
    let walked = Walked::open(formula.theory()).decide(2, true);
    assert_eq!(
        walked.scores(),
        [Some(shortening(3, 2)), Some(shortening(3, 2)), None]
    );
}

#[test]
fn an_occurrence_under_three_implications_is_not_read() {
    // The first constraint holds 0 doubly negated and the second holds 3
    // triply negated; each lost its last operand.
    let mut formula = Formula::new(6);
    let negations: Vec<usize> = (0..6).map(|atom| formula.choice(atom, None)).collect();
    let twice = formula.not(negations[0]);
    formula.never(&[twice, 1, 2]);
    let twice = formula.not(negations[3]);
    let thrice = formula.not(twice);
    formula.never(&[thrice, 4, 5]);
    let walked = Walked::open(formula.theory())
        .decide(2, true)
        .decide(5, true);
    let read = Some(shortening(3, 2));
    assert_eq!(walked.scores(), [read, read, None, Some(0), read, None]);
}

#[test]
fn a_known_implication_is_not_read_through() {
    // No answer set satisfies `0 → 1` and holds 2 and 3. Holding 1 settles
    // the implication whatever 0 is, so the constraint it shortens says
    // nothing of 0.
    let mut formula = Formula::new(4);
    for atom in 0..4 {
        formula.choice(atom, None);
    }
    let implication = formula.push(Node::implies(0, 1));
    formula.never(&[implication, 2, 3]);
    let walked = Walked::open(formula.theory()).decide(1, true);
    assert_eq!(
        walked.scores(),
        [
            Some(0),
            None,
            Some(shortening(3, 2)),
            Some(shortening(3, 2))
        ]
    );
}

#[test]
fn an_atom_scores_once_for_each_occurrence_in_a_chain() {
    // No answer set holds 0, its double negation, 1 and 2 together. With 2
    // held the constraint has three members left, two of them occurrences
    // of 0.
    let mut formula = Formula::new(3);
    let absent = formula.choice(0, None);
    formula.choice(1, None);
    formula.choice(2, None);
    let twice = formula.not(absent);
    formula.never(&[0, twice, 1, 2]);
    let walked = Walked::open(formula.theory()).decide(2, true);
    assert_eq!(
        walked.scores(),
        [Some(2 * shortening(4, 3)), Some(shortening(4, 3)), None]
    );
}

#[test]
fn open_atoms_are_ranked_alike_once_no_generating_head_is_open() {
    // 0 is the one choice. 1 and 2 exclude each other by default negation, as
    // 3 and 4 do, so none of the four is a generating head. With 0 held the
    // constraint on 0, 1 and 2 has two members left, and one of them is taken
    // before the earlier component of 3 and 4.
    let mut formula = Formula::new(5);
    formula.choice(0, None);
    for (head, other) in [(1, 2), (2, 1), (3, 4), (4, 3)] {
        let absent = formula.not(other);
        let rule = formula.push(Node::implies(absent, head));
        formula.roots.push(rule);
    }
    formula.never(&[0, 1, 2]);
    let walked = Walked::open(formula.theory());
    let ranks = walked.order.ranks.clone();
    assert!(ranks[3].min(ranks[4]) < ranks[1].min(ranks[2]));
    let walked = walked.decide(0, true);
    assert_eq!(
        walked.scores(),
        [
            None,
            Some(shortening(3, 2)),
            Some(shortening(3, 2)),
            Some(0),
            Some(0)
        ]
    );
    let first = [1, 2]
        .into_iter()
        .min_by_key(|&atom| (ranks[atom], atom))
        .unwrap();
    assert_eq!(walked.preferred, Some(first));
}

#[test]
fn a_lesser_depth_precedes_a_shortened_chain() {
    // 2, 3 and 4 are chosen under 0, so they lie one step above 0 and 1; no
    // answer set holds all three.
    let mut formula = Formula::new(5);
    formula.choice(0, None);
    formula.choice(1, None);
    for atom in 2..5 {
        formula.choice(atom, Some(0));
    }
    formula.never(&[2, 3, 4]);
    let walked = Walked::open(formula.theory());
    assert_eq!(walked.order.depths, [0, 0, 1, 1, 1]);
    let walked = walked.decide(0, true).decide(4, true);
    assert_eq!(
        walked.scores(),
        [
            None,
            Some(0),
            Some(shortening(3, 2)),
            Some(shortening(3, 2)),
            None
        ]
    );
    assert_eq!(walked.preferred, Some(1));
}

/// The reads of the selection in `a_shortened_chain_precedes_an_earlier_component`
/// after 0 is held: the open heads 1, 2 and 3 once each; then once more with
/// their parents. 3 has its choice and its negation, and through the
/// negation the choice again: three. 1 and 2 have the constraint besides:
/// four each.
const SELECTION_WORK: u64 = 3 + 3 + 3 + 4 + 4;

#[test]
fn a_shortened_selection_has_the_receipt_of_its_reads() {
    let walked = Walked::open(constrained(4, &[&[0, 1, 2]])).decide(0, true);
    let mut work = Meter::local(u64::MAX);
    let selected = walked
        .order
        .preferred(
            &walked.narrower,
            &walked.theory,
            &walked.region,
            &walked.known,
            &mut work,
        )
        .map_err(zetesis_algebra::Failure::into_stop);
    assert_eq!(selected, Ok(Some(2)));
    assert_eq!(work.charged(), SELECTION_WORK);
}

#[test]
fn a_refused_shortened_selection_publishes_no_partial_winner() {
    let walked = Walked::open(constrained(4, &[&[0, 1, 2]])).decide(0, true);
    for limit in 0..=SELECTION_WORK + 1 {
        let mut quota = Allowance {
            remaining: limit,
            batch: 5,
            spent: 0,
        };
        let attempt = crate::test_support::stop_preference(&walked.order.preferred_atom_reserved(
            &walked.narrower,
            &walked.theory,
            &walked.region,
            &walked.known,
            &Cancellation::default(),
            &mut zetesis_algebra::Meter::reserved(&mut quota),
        ));
        assert_eq!(attempt.work, limit.min(SELECTION_WORK), "limit {limit}");
        assert_eq!(quota.spent, attempt.work, "limit {limit}");
        assert_eq!(
            attempt.result,
            if limit < SELECTION_WORK {
                Err(Stop::WorkLimit)
            } else {
                Ok(Some(2))
            },
            "limit {limit}"
        );
    }
}

#[test]
fn an_order_refuses_an_index_of_another_universe() {
    let walked = Walked::open(constrained(4, &[&[0, 1, 2]]));
    let other = Narrower::new(&constrained(5, &[]));
    let mut work = Meter::local(u64::MAX);
    assert_eq!(
        walked
            .order
            .preferred(
                &other,
                &walked.theory,
                &walked.region,
                &walked.known,
                &mut work
            )
            .map_err(zetesis_algebra::Failure::into_stop),
        Err(Stop::InvalidProgram)
    );
    assert_eq!(work.charged(), 0);
}
