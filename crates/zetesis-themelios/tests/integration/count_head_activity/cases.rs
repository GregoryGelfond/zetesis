//! Original count-head aliases paired with explicit permission/constraint forms.

pub(super) const CASES: &[(&str, &str)] = &[
    ("1#count{1:a;2:a}1.", "{a}.:-not 1#count{1:a;2:a}1."),
    ("2#count{1:a;2:a}2.", "{a}.:-not 2#count{1:a;2:a}2."),
    ("1#count{1:a;1:b}1.", "{a;b}.:-not 1#count{1:a;1:b}1."),
    ("0#count{1:a;1:b}0.", "{a;b}.:-not 0#count{1:a;1:b}0."),
    ("2#count{1:a;1:b}2.", "{a;b}.:-not 2#count{1:a;1:b}2."),
    (
        "1#count{1:a;1:b;2:c}1.",
        "{a;b;c}.:-not 1#count{1:a;1:b;2:c}1.",
    ),
    (
        "2#count{1:a;2:b;3:a}2.",
        "{a;b}.:-not 2#count{1:a;2:b;3:a}2.",
    ),
    ("1#count{1:a;1:a;1:b}1.", "{a;b}.:-not 1#count{1:a;1:b}1."),
    ("#count{1:a;2:a}.", "{a}."),
    ("#count{1:a;1:b}.", "{a;b}."),
    ("1#count{X:a:X=1..2}1.", "{a}.:-not 1#count{1:a;2:a}1."),
    (
        "1#count{1:a(1..2)}1.",
        "{a(1..2)}.:-not 1#count{1:a(1);1:a(2)}1.",
    ),
    (
        "d(1..2).2#count{X:a:d(X)}2.",
        "d(1..2).{a}.:-not 2#count{X:a,d(X)}2.",
    ),
    (
        "{b;c}.1#count{1:a:b;1:d:c}1.",
        "{b;c}. {a:b;d:c}.:-not 1#count{1:a,b;1:d,c}1.",
    ),
    (
        "{b;c}.1#count{1:a:b;2:a:c}1.",
        "{b;c}. {a:b;a:c}.:-not 1#count{1:a,b;2:a,c}1.",
    ),
    (
        "{b}.1#count{1:a:b;1:c:not b}1.",
        "{b}. {a:b;c:not b}.:-not 1#count{1:a,b;1:c,not b}1.",
    ),
    (
        "{b}.1#count{1:a:not b;2:a:not not b}1.",
        "{b}. {a:not b;a:not not b}.:-not 1#count{1:a,not b;2:a,not not b}1.",
    ),
    (
        "b:-a.1#count{1:a:b;2:a:not b}1.",
        "b:-a.{a:b;a:not b}.:-not 1#count{1:a,b;2:a,not b}1.",
    ),
    (
        "1#count{1:a:not not a;1:b:not not b}1.",
        "{a:not not a;b:not not b}.:-not 1#count{1:a,not not a;1:b,not not b}1.",
    ),
    (
        "1#count{1:a:a;1:b:b}1.",
        "{a:a;b:b}.:-not 1#count{1:a,a;1:b,b}1.",
    ),
    (
        "{e}.1#count{1:a;1:b}1:-e.",
        "{e}.{a;b}:-e.:-e,not 1#count{1:a;1:b}1.",
    ),
    (
        "{e}.1#count{1:a;2:a}1:-not e.",
        "{e}.{a}:-not e.:-not e,not 1#count{1:a;2:a}1.",
    ),
    (
        "e:-a.1#count{1:a;1:b}1:-e.",
        "e:-a.{a;b}:-e.:-e,not 1#count{1:a;1:b}1.",
    ),
    ("1#count{1: -a;1:b}1.", "{-a;b}.:-not 1#count{1: -a;1:b}1."),
    ("1#count{1:a;1: -a}1.", "{a;-a}.:-not 1#count{1:a;1: -a}1."),
    (
        "1#count{k(1),\"x\":a;k(1),\"x\":b}1.",
        "{a;b}.:-not 1#count{k(1),\"x\":a;k(1),\"x\":b}1.",
    ),
    ("2#count{1,x:a;1,y:a}2.", "{a}.:-not 2#count{1,x:a;1,y:a}2."),
    ("#count{:a;:b}=1.", "{a;b}.:-not #count{:a;:b}=1."),
    ("#count{1:a;1:b}!=1.", "{a;b}.:-not #count{1:a;1:b}!=1."),
    ("#count{1:a;2:a}<2.", "{a}.:-not #count{1:a;2:a}<2."),
    ("#count{1:a;1:b}>0.", "{a;b}.:-not #count{1:a;1:b}>0."),
    ("#count{1:a;2:a}<=1.", "{a}.:-not #count{1:a;2:a}<=1."),
    ("#count{1:a;2:a}>=1.", "{a}.:-not #count{1:a;2:a}>=1."),
    (
        "1#count{1:a:b;1:c:d}1.b.d:-b.",
        "b.d:-b.{a:b;c:d}.:-not 1#count{1:a,b;1:c,d}1.",
    ),
    (
        "#count{1:a:missing;1:b}<=1.",
        "{a:missing;b}.:-not #count{1:a,missing;1:b}<=1.",
    ),
];
