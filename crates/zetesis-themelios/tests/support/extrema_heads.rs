//! Independent head permissions and activated body-extremum constraints.

pub(super) fn sources() -> Vec<(String, String)> {
    let mut cases: Vec<_> = CASES.iter().map(|&(a, b)| (a.into(), b.into())).collect();
    for function in ["#min", "#max"] {
        for relation in ["=", "!=", "<", "<=", ">", ">="] {
            for tuples in ["", "-1:a;0:b;2:c"] {
                let aggregate = format!("{function}{{{tuples}}}{relation}0");
                let permissions = if tuples.is_empty() { "" } else { "{a;b;c}." };
                cases.push((
                    format!("{aggregate}."),
                    format!("{permissions}:-not {aggregate}."),
                ));
            }
        }
    }
    cases
}

const CASES: &[(&str, &str)] = &[
    ("1#min{1:a;1:b}1.", "{a;b}.:-not 1#min{1:a;1:b}1."),
    ("1#max{1:a;1:b}1.", "{a;b}.:-not 1#max{1:a;1:b}1."),
    ("1#min{1:a;2:a}1.", "{a}.:-not 1#min{1:a;2:a}1."),
    ("2#max{1:a;2:a}2.", "{a}.:-not 2#max{1:a;2:a}2."),
    (
        "{b;c}.1#min{1,k:a:b;1,k:d:c;2,l:a:c}1.",
        "{b;c}.{a:b;d:c;a:c}.:-not 1#min{1,k:a,b;1,k:d,c;2,l:a,c}1.",
    ),
    (
        "{b;c}.2#max{1,k:a:b;1,k:d:c;2,l:a:c}2.",
        "{b;c}.{a:b;d:c;a:c}.:-not 2#max{1,k:a,b;1,k:d,c;2,l:a,c}2.",
    ),
    (
        "0<=#min{0:a:a;0:a:not a}.",
        "{a:a;a:not a}.:-not 0<=#min{0:a,a;0:a,not a}.",
    ),
    ("1#min{1,k:a;1,l:b}1.", "{a;b}.:-not 1#min{1,k:a;1,l:b}1."),
    ("1#max{1,k:a;1,l:b}1.", "{a;b}.:-not 1#max{1,k:a;1,l:b}1."),
    (
        "d(0..1).I#min{I:p(I)}I:-d(I).",
        "d(0..1).{p(0)}:-d(0).{p(1)}:-d(1).:-d(0),not 0#min{0:p(0)}0.:-d(1),not 1#min{1:p(1)}1.",
    ),
    (
        "#min{2147483646:a}=2147483646.",
        "{a}.:-not #min{2147483646:a}=2147483646.",
    ),
    (
        "#max{-2147483647:a}=-2147483647.",
        "{a}.:-not #max{-2147483647:a}=-2147483647.",
    ),
    ("0#min{0:a}0.", "{a}.:-not 0#min{0:a}0."),
    ("0#max{0:a}0.", "{a}.:-not 0#max{0:a}0."),
    ("1#min{1:a}1.", "{a}.:-not 1#min{1:a}1."),
    ("1#max{1:a}1.", "{a}.:-not 1#max{1:a}1."),
    ("#min{1:a}=#sup.", "{a}.:-not #min{1:a}=#sup."),
    ("#max{1:a}=#inf.", "{a}.:-not #max{1:a}=#inf."),
    ("#min{1:a}<=word.", "{a}.:-not #min{1:a}<=word."),
    ("#max{1:a}>=word.", "{a}.:-not #max{1:a}>=word."),
    ("-1#min{-2:a;1:b}2.", "{a;b}.:-not -1#min{-2:a;1:b}2."),
    ("2#max{-2:a;1:b}0.", "{a;b}.:-not 2#max{-2:a;1:b}0."),
    (
        "{d}.0<=#min{0:a:not d}.",
        "{d}.{a:not d}.:-not 0<=#min{0:a,not d}.",
    ),
    (
        "0<=#min{0:a:not not a}.",
        "{a:not not a}.:-not 0<=#min{0:a,not not a}.",
    ),
    ("0<=#max{0:a:a}.", "{a:a}.:-not 0<=#max{0:a}."),
    (
        "{b;c}.1#min{1,k:a:b;1,k:a:c}1.",
        "{b;c}.{a:b;a:c}.:-not 1#min{1,k:a,b;1,k:a,c}1.",
    ),
    (
        "d(-1..1).0#max{X:p(X):d(X)}1.",
        "d(-1..1).{p(X):d(X)}.:-not 0#max{X:p(X),d(X)}1.",
    ),
    (
        "{e}.2#min{2:a;3:b}2:-e.",
        "{e}.{a;b}:-e.:-e,not 2#min{2:a;3:b}2.",
    ),
    (
        "{d}.N#max{N:a}N:-N=#count{1:d}.",
        "{d}.{a}:-0=#count{1:d}.{a}:-1=#count{1:d}.:-0=#count{1:d},not 0#max{0:a}0.:-1=#count{1:d},not 1#max{1:a}1.",
    ),
    (
        "{d}.Y#min{Y:a}Y:-N=#count{1:d},Y=N+1.",
        "{d}.{a}:-0=#count{1:d}.{a}:-1=#count{1:d}.:-0=#count{1:d},not 1#min{1:a}1.:-1=#count{1:d},not 2#min{2:a}2.",
    ),
    (
        "1#min{1:a:missing}1.",
        "{a:missing}.:-not 1#min{1:a,missing}1.",
    ),
    ("1#max{}1:-#false.", ":-#false,not 1#max{}1."),
    ("a.0#min{0: -a}0.", "a.{-a}.:-not 0#min{0: -a}0."),
];
