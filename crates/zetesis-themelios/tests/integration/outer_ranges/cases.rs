//! Authored expansions keep each outer range row with its original equality.

pub(super) const CASES: &[(&str, &str)] = &[
    ("q(K):-N=#count{},K=1..N.", ""),
    (
        "q(K):-N=#count{},K=N..N+1.",
        "q(0):-0=#count{}.q(1):-0=#count{}.",
    ),
    ("q(K):-N+1..N=K,N=#count{}.", ""),
    (
        "q(K):-N..N+1=K,N=#count{}.",
        "q(0):-0=#count{}.q(1):-0=#count{}.",
    ),
    (
        "q(K):-K=Y..Y+1,Y=N+1,N=#count{}.",
        "q(1):-0=#count{}.q(2):-0=#count{}.",
    ),
    (
        "{d}.q(K):-N=#count{1:d},K=1..N.",
        "{d}.q(1):-1=#count{1:d}.",
    ),
    (
        "{d}.q(K):-K=N..N+1,N=#count{1:d}.",
        "{d}.q(0):-0=#count{1:d}.q(1):-0=#count{1:d}.q(1):-1=#count{1:d}.q(2):-1=#count{1:d}.",
    ),
    (
        "{d}.q(K):-N=#sum{-1:d},K=N..0.",
        "{d}.q(0):-0=#sum{-1:d}.q(-1):--1=#sum{-1:d}.q(0):--1=#sum{-1:d}.",
    ),
    (
        "{d}.q(K):-N=#sum+{-1:d;2:d},K=1..N.",
        "{d}.q(1):-2=#sum+{-1:d;2:d}.q(2):-2=#sum+{-1:d;2:d}.",
    ),
    ("{d}.q(K):-N=#min{2:d},K=N..N.", "{d}.q(2):-2=#min{2:d}."),
    ("{d}.q(K):-N=#max{2:d},K=N..N.", "{d}.q(2):-2=#max{2:d}."),
    ("{d}.q(K):-N=#min{a:d},K=N..N.", "{d}."),
    ("q(K):-N=#count{},X=f(N),K=X..X.", ""),
    (
        "i(1..2).q(I,K):-i(I),N=#count{},K=N..N+1.",
        "i(1..2).q(1,0):-0=#count{}.q(1,1):-0=#count{}.q(2,0):-0=#count{}.q(2,1):-0=#count{}.",
    ),
    (
        "{d}.q(N,K):-N=#count{1:d},M=#sum{1:d},K=M..M.",
        "{d}.q(0,0):-0=#count{1:d},0=#sum{1:d}.q(0,1):-0=#count{1:d},1=#sum{1:d}.q(1,0):-1=#count{1:d},0=#sum{1:d}.q(1,1):-1=#count{1:d},1=#sum{1:d}.",
    ),
    (
        "{d}.q(K,L):-N=#count{1:d},K=N..N+1,L=K..K+1.",
        "{d}.q(0,0):-0=#count{1:d}.q(0,1):-0=#count{1:d}.q(1,1):-0=#count{1:d}.q(1,2):-0=#count{1:d}.q(1,1):-1=#count{1:d}.q(1,2):-1=#count{1:d}.q(2,2):-1=#count{1:d}.q(2,3):-1=#count{1:d}.",
    ),
    (
        "d(0..2).q(K):-d(K),N=#count{},K=N..N+1.",
        "d(0..2).q(0):-0=#count{}.q(1):-0=#count{}.",
    ),
    (
        "d(0..2).q(K):-K=N..N+1,N=#count{},d(K).",
        "d(0..2).q(0):-0=#count{}.q(1):-0=#count{}.",
    ),
    (
        "{d}.q(K):-N=#count{1:d},K=0..2,K=N..N+1.",
        "{d}.q(0):-0=#count{1:d}.q(1):-0=#count{1:d}.q(1):-1=#count{1:d}.q(2):-1=#count{1:d}.",
    ),
    (
        "{p(0)}.q(K):-N=#count{},K=N..N+1,not p(K).",
        "{p(0)}.q(0):-0=#count{},not p(0).q(1):-0=#count{},not p(1).",
    ),
    (
        "{p(0,a)}.q(K):-not not p(K,_),K=N..N+1,N=#count{}.",
        "{p(0,a)}.q(0):-0=#count{},not not p(0,a).q(1):-0=#count{},not not p(1,_).",
    ),
    (
        "{d}.1{p(K);q(K)}1:-N=#count{1:d},K=1..N.",
        "{d}.1{p(1);q(1)}1:-1=#count{1:d}.",
    ),
    (
        "{d}.1#count{1:p(K);2:q(K)}1:-K=1..N,N=#count{1:d}.",
        "{d}.1{p(1);q(1)}1:-1=#count{1:d}.",
    ),
    (
        "K{p;q}K:-N=#count{},K=N..N+1.",
        "0{p;q}0:-0=#count{}.1{p;q}1:-0=#count{}.",
    ),
    (
        "K#count{1:p;2:q}K:-N=#count{},K=N..N+1.",
        "0{p;q}0:-0=#count{}.1{p;q}1:-0=#count{}.",
    ),
    (
        "{d}.q(N..N+1):-N=#count{1:d}.",
        "{d}.q(0):-0=#count{1:d}.q(1):-0=#count{1:d}.q(1):-1=#count{1:d}.q(2):-1=#count{1:d}.",
    ),
    ("r(N,M):-N=#count{},M=#count{},K=1..M.", ""),
];

// Portable endpoint controls; the external maximum-endpoint attempt stopped
// without completion and is not counted as clingo parity.
pub(super) const BOUNDARIES: &[(&str, &str)] = &[
    (
        "q(K):-N=#count{},K=2147483647+N..2147483647+N.",
        "q(2147483647):-0=#count{}.",
    ),
    (
        "q(K):-N=#count{},K=(-2147483647-1+N)..(-2147483647-1+N).",
        "q(-2147483647-1):-0=#count{}.",
    ),
];
