//! A disequality over a keyed value asks for the one atom the key admits.

#[path = "support/source_oracle.rs"]
mod source_oracle;
#[path = "support/source_records.rs"]
mod source_records;

use std::collections::BTreeSet;

use zetesis_core::Atom;
use zetesis_cpu::Control;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{AdmittedFormula, FormulaLimits};

const CHOICES: &str = "letter(a;b;c). digit(0..9). carry_value(0;1). idx(1). \
    1 { assign(L,D) : digit(D) } 1 :- letter(L). \
    1 { carry(I,V) : carry_value(V) } 1 :- idx(I). ";

fn admitted(source: &str) -> AdmittedFormula {
    source_records::admit(source, &FormulaLimits::default())
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn stable(input: &AdmittedFormula) -> BTreeSet<BTreeSet<Atom>> {
    let mut search =
        StableModels::new(input.theory(), Limits::default(), Control::default()).unwrap();
    let mut result = BTreeSet::new();
    for model in search.by_ref() {
        let atoms = model
            .unwrap()
            .atoms()
            .map(|index| input.atoms()[index].clone())
            .collect();
        assert!(result.insert(atoms));
    }
    assert!(search.exhausted());
    result
}

/// The two programs ground to theories of the same shape and the same family.
fn same_shape(left: &str, right: &str) {
    let (left, right) = (admitted(left), admitted(right));
    assert_eq!(left.atoms(), right.atoms());
    assert_eq!(left.theory().roots().len(), right.theory().roots().len());
    assert_eq!(left.theory().nodes().len(), right.theory().nodes().len());
    assert_eq!(stable(&left), stable(&right));
}

#[test]
fn a_disequality_over_one_keyed_value_asks_for_the_one_atom() {
    // X is assigned exactly one digit, so "no digit but Y+1" is "the digit
    // Y+1 is missing": ten constraint instances where the product had ninety.
    let written = format!("{CHOICES} :- assign(a,X), assign(b,Y), X != Y + 1.");
    let asked = format!("{CHOICES} :- assign(b,Y), letter(a), not assign(a, Y + 1).");
    same_shape(&written, &asked);
    assert_eq!(admitted(&written).keyed_constraints(), 1);
    assert_eq!(admitted(&asked).keyed_constraints(), 0);
    // Nine pairs with X = Y + 1, times the free letter and the carry.
    assert_eq!(stable(&admitted(&written)).len(), 9 * 10 * 2);
}

#[test]
fn a_digit_and_carry_column_asks_for_both_atoms() {
    // A + B = C + 10 K with C a digit and K a carry has the one solution
    // C = (A + B) mod 10, K = (A + B) div 10.
    let written = format!(
        "{CHOICES} :- assign(a,A), assign(b,B), assign(c,C), carry(1,K), A + B != C + 10 * K."
    );
    let asked = format!(
        "{CHOICES} :- assign(a,A), assign(b,B), letter(c), idx(1), not assign(c, (A + B) \\ 10). \
         :- assign(a,A), assign(b,B), letter(c), idx(1), not carry(1, (A + B) / 10)."
    );
    same_shape(&written, &asked);
    assert_eq!(stable(&admitted(&written)).len(), 100);
}

#[test]
fn the_written_forms_of_the_column_are_all_read() {
    let forms = [
        "A + B != C + 10 * K",
        "A + B != 10 * K + C",
        "C + 10 * K != A + B",
        "K * 10 + C != A + B",
    ];
    let reference = format!(
        "{CHOICES} :- assign(a,A), assign(b,B), letter(c), idx(1), not assign(c, (A + B) \\ 10). \
         :- assign(a,A), assign(b,B), letter(c), idx(1), not carry(1, (A + B) / 10)."
    );
    for form in forms {
        let written =
            format!("{CHOICES} :- assign(a,A), assign(b,B), assign(c,C), carry(1,K), {form}.");
        same_shape(&written, &reference);
    }
}

#[test]
fn a_value_read_elsewhere_keeps_the_written_constraint() {
    // X also decides X > 1, so the constraint is not about one demanded
    // digit; the product form stays, and grounds to ninety instances less
    // the ones X > 1 excludes.
    let written = format!("{CHOICES} :- assign(a,X), assign(b,Y), X != Y + 1, X > 1.");
    let input = admitted(&written);
    assert_eq!(input.keyed_constraints(), 0);
    let constraints = input
        .theory()
        .roots()
        .len()
        .saturating_sub(admitted(CHOICES).theory().roots().len());
    assert_eq!(constraints, 8 * 10 - 8);
    // Twenty pairs with X <= 1 and eight with X = Y + 1 > 1 remain, times
    // the free letter and the carry.
    assert_eq!(stable(&input).len(), 28 * 10 * 2);
}

const LETTERS: &str = "letter(a;b). val(3;4). 1 { assign(K,V) : val(V) } 1 :- letter(K). ";

#[test]
fn an_anonymous_key_keeps_the_written_constraint() {
    // The written constraint forbids a value other than 3 at any key. Asked
    // by key it would read `not assign(_, 3)`, no key at all holding 3, which
    // forbids less: the key must be named for the one atom to be the key's.
    let anonymous = admitted(&format!("{LETTERS} :- assign(_, Y), Y != 3."));
    assert_eq!(anonymous.keyed_constraints(), 0);
    // Both letters hold 3, as they do when the key is named.
    let named = admitted(&format!("{LETTERS} :- assign(K, Y), Y != 3."));
    assert_eq!(stable(&anonymous).len(), 1);
    assert_eq!(stable(&anonymous), stable(&named));
}

const COLUMN: &str = "letter(a;b). digit(0..1). carry_value(0;1). idx(1). \
    1 { assign(L,D) : digit(D) } 1 :- letter(L). \
    1 { carry(I,V) : carry_value(V) } 1 :- idx(I). ";

#[test]
fn an_anonymous_key_keeps_the_written_column() {
    // As above for the digit and the carry: 3 = C + 2K over C and K in 0..1
    // has the one solution C = 1, K = 1, and with the digit's key anonymous
    // every letter's digit must be that one, not some letter's.
    let anonymous = admitted(&format!(
        "{COLUMN} :- assign(_, C), carry(1, K), 3 != C + 2 * K."
    ));
    assert_eq!(anonymous.keyed_constraints(), 0);
    let named = admitted(&format!(
        "{COLUMN} :- assign(L, C), carry(1, K), 3 != C + 2 * K."
    ));
    assert_eq!(stable(&anonymous).len(), 1);
    assert_eq!(stable(&anonymous), stable(&named));
}

#[test]
fn a_relation_with_another_producer_keeps_the_written_constraint() {
    let choices = format!("{CHOICES} assign(a,0) :- not assign(a,1).");
    let written = format!("{choices} :- assign(a,X), assign(b,Y), X != Y + 1.");
    let asked = format!("{choices} :- assign(b,Y), letter(a), not assign(a, Y + 1).");
    let (written, asked) = (admitted(&written), admitted(&asked));
    assert_eq!(written.keyed_constraints(), 0);
    assert!(written.theory().roots().len() > asked.theory().roots().len());
}

#[test]
fn a_digit_outside_the_carry_base_keeps_the_written_column() {
    // With digits up to 12 the column has more than one solution, so the
    // inversion does not apply and the product form stays.
    let choices = CHOICES.replace("digit(0..9)", "digit(0..12)");
    let written = format!(
        "{choices} :- assign(a,A), assign(b,B), assign(c,C), carry(1,K), A + B != C + 10 * K."
    );
    let asked = format!(
        "{choices} :- assign(a,A), assign(b,B), letter(c), idx(1), not assign(c, (A + B) \\ 10). \
         :- assign(a,A), assign(b,B), letter(c), idx(1), not carry(1, (A + B) / 10)."
    );
    let (written, asked) = (admitted(&written), admitted(&asked));
    assert_eq!(written.keyed_constraints(), 0);
    assert!(written.theory().roots().len() > asked.theory().roots().len());
    assert_ne!(stable(&written), stable(&asked));
}

#[test]
fn asked_constraints_keep_the_written_constraint_as_their_origin() {
    let written = format!("{CHOICES} :- assign(a,X), assign(b,Y), X != Y + 1.");
    let start = written.find(":- assign(a,X)").unwrap();
    let input = admitted(&written);
    let spans: BTreeSet<_> = input
        .formula_origins()
        .iter()
        .flatten()
        .map(|origin| origin.span.start().get())
        .collect();
    assert!(spans.contains(&u32::try_from(start).unwrap()), "{spans:?}");
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn every_interpretation_of_a_small_asked_program_matches_clingo() {
    let source = "letter(a;b). digit(0..1). 1 { assign(L,D) : digit(D) } 1 :- letter(L). \
        :- assign(a,X), assign(b,Y), X != Y.";
    let admitted = admitted(source);
    assert_eq!(
        source_records::exhaustive(&admitted),
        source_oracle::records(source),
        "{source}"
    );
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn a_constraint_with_an_anonymous_key_matches_clingo_as_written() {
    // The rewrite declines an anonymous key, so the written constraint
    // decides the family: both letters hold 3, and the column has its one
    // solution at every letter.
    for source in [
        format!("{LETTERS} :- assign(_, Y), Y != 3."),
        format!("{COLUMN} :- assign(_, C), carry(1, K), 3 != C + 2 * K."),
    ] {
        assert_eq!(admitted(&source).keyed_constraints(), 0, "{source}");
        let family: BTreeSet<_> = stable(&admitted(&source))
            .into_iter()
            .map(|atoms| (atoms.iter().map(source_records::canonical).collect(), None))
            .collect();
        assert_eq!(family, source_oracle::records(&source), "{source}");
    }
}

#[test]
#[ignore = "requires independent clingo 5.8.2"]
fn asked_constraints_match_clingo() {
    // Four digits keep the families within the oracle's capture; the digits
    // still lie within the column's base.
    let choices = CHOICES.replace("digit(0..9)", "digit(0..3)");
    for source in [
        format!("{choices} :- assign(a,X), assign(b,Y), X != Y + 1."),
        format!(
            "{choices} :- assign(a,A), assign(b,B), assign(c,C), carry(1,K), A + B != C + 10 * K."
        ),
        format!("{choices} :- assign(a,X), carry(1,K), K != X."),
    ] {
        assert_eq!(admitted(&source).keyed_constraints(), 1, "{source}");
        let family: BTreeSet<_> = stable(&admitted(&source))
            .into_iter()
            .map(|atoms| (atoms.iter().map(source_records::canonical).collect(), None))
            .collect();
        assert_eq!(family, source_oracle::records(&source), "{source}");
    }
}
