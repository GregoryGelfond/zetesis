//! Handwritten choice permissions and independently activated body constraints.

pub(super) const CASES: &[(&str, &str)] = &[
    ("1#sum{1:a}1.", "{a}.:-not 1#sum{1:a}1."),
    ("0#sum{0:a}0.", "{a}.:-not 0#sum{0:a}0."),
    ("0#sum+{0:a}0.", "{a}.:-not 0#sum+{0:a}0."),
    ("0#sum{}0.", ":-not 0#sum{}0."),
    ("1#sum{}1.", ":-not 1#sum{}1."),
    ("0#sum+{}0.", ":-not 0#sum+{}0."),
    (
        "1#sum{X:p(X):X=1..4}2.",
        "{p(X):X=1..4}.:-not 1#sum{X:p(X),X=1..4}2.",
    ),
    ("2#sum{1:a;2:b}2.", "{a;b}.:-not 2#sum{1:a;2:b}2."),
    ("1#sum{-1:a;2:b}1.", "{a;b}.:-not 1#sum{-1:a;2:b}1."),
    ("#sum{1:a;2:b}<=2.", "{a;b}.:-not #sum{1:a;2:b}<=2."),
    ("1<=#sum{1:a;2:b}.", "{a;b}.:-not 1<=#sum{1:a;2:b}."),
    ("#sum{1:a;2:b}!=2.", "{a;b}.:-not #sum{1:a;2:b}!=2."),
    (
        "{d}.0#sum{0:a:not d}0.",
        "{d}.{a:not d}.:-not 0#sum{0:not d,a}0.",
    ),
    (
        "0#sum{0:a:not not a}0.",
        "{a:not not a}.:-not 0#sum{0:not not a,a}0.",
    ),
    ("0#sum{0:a:a}0.", "{a:a}.:-not 0#sum{0:a}0."),
    (
        "{b;c}.1#sum{1,k:a:b;1,k:a:c}1.",
        "{b;c}.{a:b;a:c}.:-not 1#sum{1,k:b,a;1,k:c,a}1.",
    ),
    (
        "d(1..2).3#sum{W:p(W):d(W)}3.",
        "d(1..2).{p(W):d(W)}.:-not 3#sum{W:d(W),p(W)}3.",
    ),
    (
        "{d}.N#sum{N:a}N:-N=#count{1:d}.",
        "{d}.{a}:-0=#count{1:d}.{a}:-1=#count{1:d}.:-0=#count{1:d},not 0#sum{0:a}0.:-1=#count{1:d},not 1#sum{1:a}1.",
    ),
    (
        "{d}.Y#sum{Y:a}Y:-N=#count{1:d},Y=N+1.",
        "{d}.{a}:-0=#count{1:d}.{a}:-1=#count{1:d}.:-0=#count{1:d},not 1#sum{1:a}1.:-1=#count{1:d},not 2#sum{2:a}2.",
    ),
    (
        "{e}.2#sum{2:a;3:b}2:-e.",
        "{e}.{a;b}:-e.:-e,not 2#sum{2:a;3:b}2.",
    ),
    ("a.0#sum{0:a}0.", "a.{a}.:-not 0#sum{0:a}0."),
    ("a.0#sum{0: -a}0.", "a.{-a}.:-not 0#sum{0: -a}0."),
    ("3#sum{1:a}1.", "{a}.:-not 3#sum{1:a}1."),
    (
        "1#sum{1:a:missing}1.",
        "{a:missing}.:-not 1#sum{1:missing,a}1.",
    ),
    (
        "{d}.1#sum+{1:a:not d}1.",
        "{d}.{a:not d}.:-not 1#sum+{1:not d,a}1.",
    ),
    ("0#sum{-1,k:a;1,k:b}0.", "{a;b}.:-not 0#sum{-1,k:a;1,k:b}0."),
    (
        "0#sum{0:a}0:-#false.",
        "{a}:-#false.:-#false,not 0#sum{0:a}0.",
    ),
    ("1#sum{1:a;1:b}1.", "{a;b}.:-not 1#sum{1:a;1:b}1."),
    ("3#sum{1:a;2:a}3.", "{a}.:-not 3#sum{1:a;2:a}3."),
    ("1#sum{1:a;2:a}1.", "{a}.:-not 1#sum{1:a;2:a}1."),
    ("2#sum{1,k:a;1,l:a}2.", "{a}.:-not 2#sum{1,k:a;1,l:a}2."),
    ("1#sum+{1:a;1:b}1.", "{a;b}.:-not 1#sum+{1:a;1:b}1."),
    ("3#sum+{1:a;2:a}3.", "{a}.:-not 3#sum+{1:a;2:a}3."),
    ("0#sum{0:a;0:b}0.", "{a;b}.:-not 0#sum{0:a;0:b}0."),
    ("0#sum+{0:a;0:b}0.", "{a;b}.:-not 0#sum+{0:a;0:b}0."),
    ("0#sum+{0,k:a;0,l:a}0.", "{a}.:-not 0#sum+{0,k:a;0,l:a}0."),
    ("0#sum{-1:a;1:a}0.", "{a}.:-not 0#sum{-1:a;1:a}0."),
    ("-1#sum{-1:a;-1:b}-1.", "{a;b}.:-not -1#sum{-1:a;-1:b}-1."),
    ("1#sum{1:a;1:a;1:b}1.", "{a;b}.:-not 1#sum{1:a;1:b}1."),
    (
        "3#sum{1:a;1:b;2:a;2:c}3.",
        "{a;b;c}.:-not 3#sum{1:a;1:b;2:a;2:c}3.",
    ),
    ("#sum{1:a;2:a}.", "{a}."),
    ("#sum+{0:a;0:b}.", "{a;b}."),
    ("#sum{1:a;1:b}!=1.", "{a;b}.:-not #sum{1:a;1:b}!=1."),
    ("#sum{1:a;2:a}<3.", "{a}.:-not #sum{1:a;2:a}<3."),
    ("#sum+{1:a;1:b}>0.", "{a;b}.:-not #sum+{1:a;1:b}>0."),
    ("#sum{1:a;2:a}<=1.", "{a}.:-not #sum{1:a;2:a}<=1."),
    ("#sum+{1:a;1:b}>=1.", "{a;b}.:-not #sum+{1:a;1:b}>=1."),
    (
        "{b;c}.1#sum{1:a:b;1:d:c}1.",
        "{b;c}.{a:b;d:c}.:-not 1#sum{1:a,b;1:d,c}1.",
    ),
    (
        "{b;c}.1#sum{1:a:b;2:a:c}1.",
        "{b;c}.{a:b;a:c}.:-not 1#sum{1:a,b;2:a,c}1.",
    ),
    (
        "{b}.1#sum+{1:a:b;1:c:not b}1.",
        "{b}.{a:b;c:not b}.:-not 1#sum+{1:a,b;1:c,not b}1.",
    ),
    (
        "{b}.1#sum{1:a:not b;2:a:not not b}1.",
        "{b}.{a:not b;a:not not b}.:-not 1#sum{1:a,not b;2:a,not not b}1.",
    ),
    (
        "b:-a.1#sum{1:a:b;2:a:not b}1.",
        "b:-a.{a:b;a:not b}.:-not 1#sum{1:a,b;2:a,not b}1.",
    ),
    (
        "1#sum{1:a:not not a;1:b:not not b}1.",
        "{a:not not a;b:not not b}.:-not 1#sum{1:a,not not a;1:b,not not b}1.",
    ),
    ("1#sum{1:a:a;1:b:b}1.", "{a:a;b:b}.:-not 1#sum{1:a;1:b}1."),
    (
        "{e}.1#sum{1:a;1:b}1:-e.",
        "{e}.{a;b}:-e.:-e,not 1#sum{1:a;1:b}1.",
    ),
    (
        "{e}.3#sum+{1:a;2:a}3:-not e.",
        "{e}.{a}:-not e.:-not e,not 3#sum+{1:a;2:a}3.",
    ),
    (
        "e:-a.1#sum{1:a;1:b}1:-e.",
        "e:-a.{a;b}:-e.:-e,not 1#sum{1:a;1:b}1.",
    ),
    ("1#sum{1: -a;1:b}1.", "{-a;b}.:-not 1#sum{1: -a;1:b}1."),
    ("1#sum{1:a;1: -a}1.", "{a;-a}.:-not 1#sum{1:a;1: -a}1."),
    (
        "2#sum{1,k(1),\"x\":a;1,k(1),\"y\":a}2.",
        "{a}.:-not 2#sum{1,k(1),\"x\":a;1,k(1),\"y\":a}2.",
    ),
    ("3#sum{X:a:X=1..2}3.", "{a}.:-not 3#sum{1:a;2:a}3."),
    (
        "1#sum+{1:a(1..2)}1.",
        "{a(1..2)}.:-not 1#sum+{1:a(1);1:a(2)}1.",
    ),
    (
        "d(1..2).3#sum{X:a:d(X)}3.",
        "d(1..2).{a}.:-not 3#sum{X:a,d(X)}3.",
    ),
    (
        "{d}.N#sum{1:a;1:b}N:-N=#count{1:d}.",
        "{d}.{a;b}:-0=#count{1:d}.{a;b}:-1=#count{1:d}.:-0=#count{1:d},not 0#sum{1:a;1:b}0.:-1=#count{1:d},not 1#sum{1:a;1:b}1.",
    ),
    (
        "a.0#sum{0:a:not a;0:b}0.",
        "a.{a:not a;b}.:-not 0#sum{0:a,not a;0:b}0.",
    ),
    (
        "d.0#sum{0,k:a:not d;0,k:b:not d}0.",
        "d.{a:not d;b:not d}.:-not 0#sum{0,k:a,not d;0,k:b,not d}0.",
    ),
    (
        "1#sum{1:a:missing;1:b}1.",
        "{a:missing;b}.:-not 1#sum{1:a,missing;1:b}1.",
    ),
    (
        "1#sum{1:a;1:b}1:-#false.",
        "{a;b}:-#false.:-#false,not 1#sum{1:a;1:b}1.",
    ),
];
