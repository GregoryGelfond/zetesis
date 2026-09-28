//! Finite substitutions retain dependent aggregate equalities in each row.

pub(super) const CASES: &[(&str, &str)] = &[
    (
        "q(N,M):-N=#count{},M=#sum{N}.",
        "q(0,0):-0=#count{},0=#sum{0}.",
    ),
    (
        "q(N,M):-M=#sum{N},N=#count{}.",
        "q(0,0):-0=#sum{0},0=#count{}.",
    ),
    (
        "{p}.q(N,M):-N=#count{1:p},M=#sum{N}.",
        "{p}.q(0,0):-0=#count{1:p},0=#sum{0}.q(1,0):-1=#count{1:p},0=#sum{1}.q(1,1):-1=#count{1:p},1=#sum{1}.",
    ),
    (
        "q(M):-N=#count{},Y=N+1,M=#sum{Y}.",
        "q(0):-0=#count{},1=0+1,0=#sum{1}.q(1):-0=#count{},1=0+1,1=#sum{1}.",
    ),
    (
        "{p}.q(N,M):-N=#sum{-1:p},M=#sum+{N}.",
        "{p}.q(-1,0):--1=#sum{-1:p},0=#sum+{-1}.q(0,0):-0=#sum{-1:p},0=#sum+{0}.",
    ),
    (
        "{p}.q(M):-N=#sum+{-1:p;1:p},M=#count{N:p}.",
        "{p}.q(0):-0=#sum+{-1:p;1:p},0=#count{0:p}.q(1):-0=#sum+{-1:p;1:p},1=#count{0:p}.q(0):-1=#sum+{-1:p;1:p},0=#count{1:p}.q(1):-1=#sum+{-1:p;1:p},1=#count{1:p}.",
    ),
    ("q(N,M):-N=#min{},M=#sum{N}.", "q(N,0):-N=#min{},0=#sum{N}."),
    (
        "q(N,M):-N=#max{},M=#sum+{N}.",
        "q(N,0):-N=#max{},0=#sum+{N}.",
    ),
    (
        "q(N,M):-N=#count{},M=#min{N}.",
        "q(0,M):-0=#count{},M=#min{0}.",
    ),
    (
        "q(N,M):-N=#count{},M=#max{N}.",
        "q(0,M):-0=#count{},M=#max{0}.",
    ),
    (
        "p(0).q(M):-N=#count{},M=#count{X:p(X),X=N}.",
        "p(0).q(0):-0=#count{},0=#count{X:p(X),X=0}.q(1):-0=#count{},1=#count{X:p(X),X=0}.",
    ),
    (
        "p(0).q(M):-N=#count{},M=#count{1:p(N)}.",
        "p(0).q(0):-0=#count{},0=#count{1:p(0)}.q(1):-0=#count{},1=#count{1:p(0)}.",
    ),
    (
        "p(f(0)).q(M):-N=#count{},M=#count{1:p(f(N))}.",
        "p(f(0)).q(0):-0=#count{},0=#count{1:p(f(0))}.q(1):-0=#count{},1=#count{1:p(f(0))}.",
    ),
    (
        "{p(0);p(1)}.q(M):-N=#count{1:p(0)},M=#count{1:p(N)}.",
        "{p(0);p(1)}.q(0):-0=#count{1:p(0)},0=#count{1:p(0)}.q(1):-0=#count{1:p(0)},1=#count{1:p(0)}.q(0):-1=#count{1:p(0)},0=#count{1:p(1)}.q(1):-1=#count{1:p(0)},1=#count{1:p(1)}.",
    ),
    (
        "p(0).q(M):-M=#count{X:p(X),not r(N,_)},N=#count{}.",
        "p(0).q(0):-0=#count{X:p(X),not r(0,_)},0=#count{}.q(1):-1=#count{X:p(X),not r(0,_)},0=#count{}.",
    ),
    (
        "{p(0)}.q(M):-N=#count{},M=#count{N:not not p(N)}.",
        "{p(0)}.q(0):-0=#count{},0=#count{0:not not p(0)}.q(1):-0=#count{},1=#count{0:not not p(0)}.",
    ),
    (
        "p(0).q(M):-N=#count{},M=#count{X:p(X),Y=N+1}.",
        "p(0).q(0):-0=#count{},0=#count{X:p(X),Y=0+1}.q(1):-0=#count{},1=#count{X:p(X),Y=0+1}.",
    ),
    (
        "p(0..1).q(M):-N=#count{},M=#count{X:p(X),X=N..N}.",
        "p(0..1).q(0):-0=#count{},0=#count{X:p(X),X=0..0}.q(1):-0=#count{},1=#count{X:p(X),X=0..0}.",
    ),
    (
        "{p}.q(M):-N=#count{},K=N..N+1,M=#count{K:p}.",
        "{p}.q(0):-0=#count{},0=#count{0:p}.q(1):-0=#count{},1=#count{0:p}.q(0):-0=#count{},0=#count{1:p}.q(1):-0=#count{},1=#count{1:p}.",
    ),
    (
        "q(K):-N=#count{},M=#sum{N},K=#sum{M}.",
        "q(0):-0=#count{},0=#sum{0},0=#sum{0}.",
    ),
    (
        "q(M):-N=#count{},Y=f(N),M=#count{Y}.",
        "q(0):-0=#count{},f(0)=f(0),0=#count{f(0)}.q(1):-0=#count{},f(0)=f(0),1=#count{f(0)}.",
    ),
    (
        "{p}.q(M):-N=#count{1:p},M=#count{N:p;0:p}.",
        "{p}.q(0):-0=#count{1:p},0=#count{0:p;0:p}.q(1):-0=#count{1:p},1=#count{0:p;0:p}.q(0):-1=#count{1:p},0=#count{1:p;0:p}.q(1):-1=#count{1:p},1=#count{1:p;0:p}.q(2):-1=#count{1:p},2=#count{1:p;0:p}.",
    ),
    (
        "{p}.q(M):-N=#count{1:p},M=#count{N:p;N:not p}.",
        "{p}.q(0):-0=#count{1:p},0=#count{0:p;0:not p}.q(1):-0=#count{1:p},1=#count{0:p;0:not p}.q(0):-1=#count{1:p},0=#count{1:p;1:not p}.q(1):-1=#count{1:p},1=#count{1:p;1:not p}.",
    ),
    (
        "{p}.M{a;b}M:-N=#count{1:p},M=#sum{N}.",
        "{p}.0{a;b}0:-0=#count{1:p},0=#sum{0}.0{a;b}0:-1=#count{1:p},0=#sum{1}.1{a;b}1:-1=#count{1:p},1=#sum{1}.",
    ),
    (
        "{p}.M#count{1:a;2:b}M:-N=#count{1:p},M=#sum{N}.",
        "{p}.0{a;b}0:-0=#count{1:p},0=#sum{0}.0{a;b}0:-1=#count{1:p},0=#sum{1}.1{a;b}1:-1=#count{1:p},1=#sum{1}.",
    ),
    (
        "{p}.q(M):-N=#count{},M=#count{N:not q(1)}.",
        "{p}.q(0):-0=#count{},0=#count{0:not q(1)}.q(1):-0=#count{},1=#count{0:not q(1)}.",
    ),
    (
        "i(1..2).q(I,M):-i(I),N=#count{},M=#sum{N}.",
        "i(1..2).q(I,0):-i(I),0=#count{},0=#sum{0}.",
    ),
    (
        "{q(1)}.q(M):-N=#count{1:q(1)},M=#count{N:q(1)}.",
        "{q(1)}.q(0):-0=#count{1:q(1)},0=#count{0:q(1)}.q(1):-0=#count{1:q(1)},1=#count{0:q(1)}.q(0):-1=#count{1:q(1)},0=#count{1:q(1)}.q(1):-1=#count{1:q(1)},1=#count{1:q(1)}.",
    ),
];
