use std::collections::BTreeSet;

use zetesis_algebra::Cancellation;
use zetesis_core::Atom;
use zetesis_ferraris::{Limits, NodeView as Node, models, models_reduct};

use crate::formula::Compiled;
use crate::formula_ground::ground;
use crate::formula_support::certain::testing::without_decided_negation;
use crate::formula_support::testing;
use crate::test_support::{answer_sets, atoms_named, compiled_atoms, interpretation};

fn compile(source: &str) -> Compiled {
    ground(testing::prepare(source), None, None).unwrap()
}

/// The theory with every default-negated body atom kept as written.
fn reference(source: &str) -> Compiled {
    without_decided_negation(|| compile(source))
}

/// Sources whose bodies negate an atom outside the support, a fact, an atom
/// of a certain predicate and an atom that is supported and not fixed, under
/// `not` and `not not`, in rules, constraints, choices and disjunctions.
const CASES: &[&str] = &[
    "d(1). p(X) :- d(X), not q(X).",
    "d(1). q(1). p(X) :- d(X), not q(X).",
    "d(1;2). q(1). p(X) :- d(X), not q(X).",
    "d(1). q(X) :- d(X). p(X) :- d(X), not q(X).",
    "d(1). {q(1)}. p(X) :- d(X), not q(X).",
    "d(1). p(X) :- d(X), not not q(X).",
    "d(1). q(1). p(X) :- d(X), not not q(X).",
    "d(1). {q(1)}. p(X) :- d(X), not not q(X).",
    "d(1). q(1). :- d(X), not q(X).",
    "d(1). :- d(X), not q(X).",
    "d(1). {q(1)}. :- d(X), not q(X).",
    "d(1). {p(X)} :- d(X), not q(X).",
    "d(1). q(1). {p(X)} :- d(X), not q(X).",
    "d(1). p(X) | r(X) :- d(X), not q(X).",
    "d(1). q(1). p(X) | r(X) :- d(X), not q(X).",
    "{a}. p :- a, not q. q :- not a.",
    "p :- not q. q :- not p.",
    "p :- not p.",
    "-q(1). d(1). p(X) :- d(X), not -q(X).",
    "d(1). p(X) :- d(X), not q(X), not r(X). r(1).",
    "d(1). h(X) :- d(X), not q(X). k(X) :- h(X), not r(X).",
    // A fact, and an atom of a certain predicate, beside a choice.
    "{p}. q :- not d,p. d.",
    "{p}. q :- not not d,p. d.",
    "c. d :- c. {p}. q :- not d, p.",
    "c. d :- c. {p}. q :- not not d, p.",
    // The negating rule stands before the rule of the predicate it negates,
    // and a reader of its head before both: the rule waits whatever the
    // order the source gives.
    "c(1). q :- d(1). d(X) :- c(X), not e(X). e(X) :- c(X).",
    "c(1;2). e(1). q :- d(2). d(X) :- c(X), not e(X).",
    // A body variable that is not in the head.
    "d(1;2). q(1). p :- d(X), not q(X). r :- p.",
    "d(1). q(1). p :- d(X), not q(X). r :- p.",
    "d(1;2). q(1). p :- d(X), not not q(X). r :- p.",
    // Two independent body components: the body is emitted by its
    // components, each of which decides as the rule's own body does.
    "{c(1;2)}. {b(1;2)}. q(1). p :- c(X), not q(X), b(Y).",
    "{c(1;2)}. {b(1;2)}. q(1). p :- c(X), not not q(X), b(Y).",
    "{c(1;2)}. {b(1;2)}. q(1). :- c(X), not q(X), b(Y).",
];

/// For each atom of `actual`, its position among `reference`'s atoms.
fn positions(actual: &[Atom], reference: &[Atom], source: &str) -> Vec<usize> {
    actual
        .iter()
        .map(|atom| {
            reference
                .iter()
                .position(|other| other == atom)
                .unwrap_or_else(|| panic!("{atom:?} is no atom of the reference: {source}"))
        })
        .collect()
}

/// `bits` over the decided theory's atoms as bits over the reference's:
/// every atom the decision left unnamed is false.
fn carried(bits: usize, positions: &[usize]) -> usize {
    positions
        .iter()
        .enumerate()
        .filter(|&(atom, _)| bits & (1 << atom) != 0)
        .fold(0, |carried, (_, &position)| carried | (1 << position))
}

#[test]
fn a_decided_negation_keeps_the_answer_sets() {
    for source in CASES {
        let actual = compile(source);
        let reference = reference(source);
        assert_eq!(
            answer_sets(&actual.theory, &actual.atoms),
            answer_sets(&reference.theory, &reference.atoms),
            "{source}"
        );
    }
}

#[test]
fn a_decided_negation_names_no_atom_the_reference_lacks() {
    for source in CASES {
        let actual = compiled_atoms(&compile(source));
        let reference = compiled_atoms(&reference(source));
        positions(&actual, &reference, source);
    }
}

#[test]
fn a_decided_negation_keeps_truth_where_the_unnamed_atoms_are_false() {
    // Every interpretation of the decided theory, read over the reference's
    // atoms with each unnamed atom false, satisfies both theories or
    // neither, and so does every interpretation tested against the reduct
    // frozen at it.
    let cancellation = Cancellation::default();
    for source in CASES {
        let actual = compile(source);
        let reference = reference(source);
        let actual_atoms = compiled_atoms(&actual);
        let reference_atoms = compiled_atoms(&reference);
        assert!(reference_atoms.len() <= 8, "bounded exhaustive population");
        let positions = positions(&actual_atoms, &reference_atoms, source);
        for frozen in 0..1_usize << actual_atoms.len() {
            let actual_frozen = interpretation(&actual.theory, frozen);
            let reference_frozen = interpretation(&reference.theory, carried(frozen, &positions));
            assert_eq!(
                models(
                    &actual.theory,
                    &actual_frozen,
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                models(
                    &reference.theory,
                    &reference_frozen,
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                "I={frozen}: {source}",
            );
            for tested in (0..1_usize << actual_atoms.len()).filter(|&tested| tested & !frozen == 0)
            {
                assert_eq!(
                    models_reduct(
                        &actual.theory,
                        &actual_frozen,
                        &interpretation(&actual.theory, tested),
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    models_reduct(
                        &reference.theory,
                        &reference_frozen,
                        &interpretation(&reference.theory, carried(tested, &positions)),
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    "I={frozen}, J={tested}: {source}",
                );
            }
        }
    }
}

#[test]
fn no_reference_answer_set_holds_an_unnamed_atom() {
    for source in CASES {
        let actual: BTreeSet<Atom> = compiled_atoms(&compile(source)).into_iter().collect();
        let reference = reference(source);
        for answer in answer_sets(&reference.theory, &reference.atoms) {
            assert!(answer.is_subset(&actual), "{answer:?}: {source}");
        }
    }
}

#[test]
fn an_atom_outside_the_support_is_not_named_under_negation() {
    let source = "d(1). p(X) :- d(X), not q(X).";
    assert!(atoms_named(&compile(source), "q").is_empty());
    assert_eq!(atoms_named(&reference(source), "q").len(), 1);
    // The literal is true: the head has an unconditional root.
    assert_eq!(atoms_named(&compile(source), "p").len(), 1);
}

#[test]
fn a_fixed_atom_under_negation_drops_its_instance() {
    // A fact and a supported atom of a certain predicate.
    for source in [
        "d(1). q(1). p(X) :- d(X), not q(X).",
        "d(1). q(X) :- d(X). p(X) :- d(X), not q(X).",
    ] {
        assert!(atoms_named(&compile(source), "p").is_empty(), "{source}");
        assert_eq!(atoms_named(&reference(source), "p").len(), 1, "{source}");
    }
}

#[test]
fn a_fixed_atom_under_double_negation_is_true() {
    let source = "d(1). q(1). p(X) :- d(X), not not q(X).";
    let actual = compile(source);
    assert_eq!(atoms_named(&actual, "p").len(), 1);
    assert!(actual.theory.nodes().len() < reference(source).theory.nodes().len());
}

#[test]
fn an_atom_outside_the_support_drops_its_instance_under_double_negation() {
    let source = "d(1). p(X) :- d(X), not not q(X).";
    assert!(atoms_named(&compile(source), "p").is_empty());
    assert!(atoms_named(&compile(source), "q").is_empty());
}

#[test]
fn a_supported_atom_that_is_not_fixed_keeps_its_literal() {
    for source in [
        "d(1). {q(1)}. p(X) :- d(X), not q(X).",
        "d(1). {q(1)}. p(X) :- d(X), not not q(X).",
        "p :- not q. q :- not p.",
    ] {
        let actual = compile(source);
        let reference = reference(source);
        assert_eq!(actual.theory.nodes(), reference.theory.nodes(), "{source}");
        assert_eq!(
            actual.theory.operands(),
            reference.theory.operands(),
            "{source}"
        );
    }
}

/// The roots of `compiled` that are implications to an atom named `name`:
/// the emitted instances of the rules that derive it.
fn instances_deriving(compiled: &Compiled, name: &str) -> usize {
    let atoms = compiled_atoms(compiled);
    let view = compiled.theory.view();
    compiled
        .theory
        .roots()
        .iter()
        .filter(|&&root| {
            matches!(view.node(root), Ok(Node::Implies(_, head))
                if matches!(view.node(head), Ok(Node::Atom(atom))
                    if atoms[atom].predicate().name() == name))
        })
        .count()
}

#[test]
fn a_body_component_decides_as_the_rules_body_does() {
    // The body has two independent components, `c(X), not q(X)` and `b(Y)`,
    // so the rule is emitted by its components: one root for its one head,
    // where the complete join has one for each of the pairs `(2, 1)` and
    // `(2, 2)`. In the first component the row of the fact `q(1)` is false,
    // and `q(2)`, outside the support, is not named.
    let source = "{c(1;2)}. {b(1;2)}. q(1). p :- c(X), not q(X), b(Y).";
    let actual = compile(source);
    assert_eq!(instances_deriving(&actual, "p"), 1);
    assert_eq!(atoms_named(&actual, "q").len(), 1);
    assert_eq!(atoms_named(&reference(source), "q").len(), 2);
}

#[test]
fn a_constraint_keeps_an_atom_outside_the_support() {
    // A streamed constraint names every atom its instances read, so the
    // emitted one keeps the literal, and with it the atom.
    let source = "d(1). :- d(X), not q(X).";
    let actual = compile(source);
    let reference = reference(source);
    assert_eq!(atoms_named(&actual, "q").len(), 1);
    assert_eq!(actual.theory.nodes(), reference.theory.nodes());
    assert_eq!(actual.theory.operands(), reference.theory.operands());
}

#[test]
fn a_constraint_drops_an_instance_whose_negated_atom_is_fixed() {
    let source = "d(1). q(1). :- d(X), not q(X).";
    let actual = compile(source);
    let reference = reference(source);
    assert_eq!(compiled_atoms(&actual), compiled_atoms(&reference));
    assert_eq!(
        actual.theory.roots().len() + 1,
        reference.theory.roots().len()
    );
}

#[test]
fn a_local_scope_keeps_its_negated_atoms() {
    // An aggregate element's condition, a choice element's condition and a
    // conditional literal's condition are emitted as written.
    for source in [
        "d(1). p :- 1 <= #count { X : d(X), not q(X) }.",
        "d(1). { p(X) : d(X), not q(X) }.",
        "d(1). p :- d(X) : d(X), not q(X).",
    ] {
        let actual = compile(source);
        let reference = reference(source);
        assert_eq!(actual.theory.nodes(), reference.theory.nodes(), "{source}");
        assert_eq!(
            actual.theory.operands(),
            reference.theory.operands(),
            "{source}"
        );
        assert_eq!(
            compiled_atoms(&actual),
            compiled_atoms(&reference),
            "{source}"
        );
    }
}

#[test]
fn an_objective_reads_a_head_through_a_decided_negation() {
    // The objective's condition reads `p`, which lies after the recursive
    // `r`, so source activity refines `r`, `q` and `p` together from the
    // completed support. It must omit the row support completion omitted:
    // `p(2)` is no supported atom, and staging it would refuse the source.
    let source = "e(1,2). b(2). r(1). r(Y) :- r(X), e(X,Y). q(X) :- r(X), b(X). \
                  p(X) :- r(X), not q(X). {c}. :~ p(X), c. [1,X]";
    let actual = compile(source);
    assert_eq!(atoms_named(&actual, "p").len(), 1);
    let reference = reference(source);
    assert_eq!(atoms_named(&reference, "p").len(), 2);
    assert_eq!(
        answer_sets(&actual.theory, &actual.atoms),
        answer_sets(&reference.theory, &reference.atoms),
    );
}
