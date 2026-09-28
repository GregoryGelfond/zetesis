//! Independent finite substitutions preserve each equality and negative gate.

pub(super) const CASES: &[(&str, &str)] = &[
    ("q(N):-N=#count{},not p(N).", "q(0):-0=#count{},not p(0)."),
    (
        "q(N):-N=#count{},not p(f(N)).",
        "q(0):-0=#count{},not p(f(0)).",
    ),
    (
        "q(N):-N=#count{},not not p(N).",
        "q(0):-0=#count{},not not p(0).",
    ),
    (
        "{p(0)}.q(N):-N=#count{},not p(N).",
        "{p(0)}.q(0):-0=#count{},not p(0).",
    ),
    (
        "{p(0)}.q(N):-not not p(N),N=#count{}.",
        "{p(0)}.q(0):-not not p(0),0=#count{}.",
    ),
    (
        "{p(1)}.q(Y):-not p(Y),Y=N+1,N=#count{}.",
        "{p(1)}.q(1):-not p(1),0=#count{}.",
    ),
    (
        "{p(f(1))}.q(Y):-N=#count{},Y=N+1,not not p(f(Y)).",
        "{p(f(1))}.q(1):-0=#count{},not not p(f(1)).",
    ),
    (
        "{-p(0)}.q(N):-N=#count{},not -p(N).",
        "{-p(0)}.q(0):-0=#count{},not -p(0).",
    ),
    (
        "{-p(0)}.q(N):-N=#count{},not not -p(N).",
        "{-p(0)}.q(0):-0=#count{},not not -p(0).",
    ),
    (
        "{d;p(0);p(1)}.q(N):-N=#count{1:d},not p(N).",
        "{d;p(0);p(1)}.q(0):-0=#count{1:d},not p(0).q(1):-1=#count{1:d},not p(1).",
    ),
    (
        "{d;p(0);p(1)}.q(N):-not not p(N),N=#count{1:d}.",
        "{d;p(0);p(1)}.q(0):-0=#count{1:d},not not p(0).q(1):-1=#count{1:d},not not p(1).",
    ),
    (
        "{d;p(-1)}.q(N):-N=#sum{-1:d},not p(N).",
        "{d;p(-1)}.q(0):-0=#sum{-1:d},not p(0).q(-1):--1=#sum{-1:d},not p(-1).",
    ),
    (
        "{d;p(2)}.q(N):-N=#sum+{-1:d;2:d},not not p(N).",
        "{d;p(2)}.q(0):-0=#sum+{-1:d;2:d},not not p(0).q(2):-2=#sum+{-1:d;2:d},not not p(2).",
    ),
    (
        "{d;p(2)}.q(N):-N=#min{2:d},not p(N).",
        "{d;p(2)}.q(V):-V=#min{},#sup=#min{2:d},not p(V).q(2):-2=#min{2:d},not p(2).",
    ),
    (
        "{d;p(2)}.q(N):-N=#max{2:d},not not p(N).",
        "{d;p(2)}.q(V):-V=#max{},#inf=#max{2:d},not not p(V).q(2):-2=#max{2:d},not not p(2).",
    ),
    (
        "{p(0,a);p(0,b)}.q(N):-N=#count{},not p(N,_).",
        "{p(0,a);p(0,b)}.q(0):-0=#count{},not p(0,a),not p(0,b).",
    ),
    (
        "{p(0,a);p(0,b)}.q(N):-N=#count{},not not p(N,_).",
        "{p(0,a);p(0,b)}.q(0):-0=#count{},not not p(0,a).q(0):-0=#count{},not not p(0,b).",
    ),
    ("q(N):-N=#count{},not p(N,_).", "q(0):-0=#count{}."),
    (
        "q(N):-N=#count{},not not p(N,_).",
        "q(0):-0=#count{},not not p(0,_).",
    ),
    (
        "{p(1,a)}.q(Y):-not p(Y,_),Y=N+1,N=#count{}.",
        "{p(1,a)}.q(1):-0=#count{},not p(1,a).",
    ),
    (
        "r(N,M):-N=#count{},M=#count{},not q(M,_).",
        "r(0,0):-0=#count{},0=#count{}.",
    ),
    (
        "{p(0)}.1{a;b}1:-N=#count{},not p(N).",
        "{p(0)}.1{a;b}1:-0=#count{},not p(0).",
    ),
    (
        "{p(0)}.1#count{1:a;2:b}1:-not not p(N),N=#count{}.",
        "{p(0)}.1{a;b}1:-not not p(0),0=#count{}.",
    ),
    (
        "{p(1,a);e}.Y{a;b}Y:-e,N=#count{},Y=N+1,not p(Y,_).",
        "{p(1,a);e}.1{a;b}1:-e,0=#count{},not p(1,a).",
    ),
    (
        "{p(1,a);e}.Y#count{1:a;2:b}Y:-Y=N+1,N=#count{},not not p(Y,_),e.",
        "{p(1,a);e}.1{a;b}1:-e,0=#count{},not not p(1,a).",
    ),
    ("p(N):-N=#count{},not p(N).", "p(0):-0=#count{},not p(0)."),
    (
        "p(N):-N=#count{},not not p(N).",
        "p(0):-0=#count{},not not p(0).",
    ),
    (
        "p(N,a):-N=#count{},not p(N,_).",
        "p(0,a):-0=#count{},not p(0,a).",
    ),
    (
        "p(N,a):-N=#count{},not not p(N,_).",
        "p(0,a):-0=#count{},not not p(0,a).",
    ),
    (
        "{d}.q(N):-N=#count{1:d},M=#sum{2:d},Y=N+M,not p(Y).",
        "{d}.q(0):-0=#count{1:d},0=#sum{2:d},not p(0).q(0):-0=#count{1:d},2=#sum{2:d},not p(2).q(1):-1=#count{1:d},0=#sum{2:d},not p(1).q(1):-1=#count{1:d},2=#sum{2:d},not p(3).",
    ),
];
