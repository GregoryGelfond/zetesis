//! Exact finite substitutions keep producer equalities and consumer polarity.

pub(super) fn sources() -> Vec<(String, String)> {
    let mut cases: Vec<_> = CASES
        .iter()
        .map(|&(source, expanded)| (source.into(), expanded.into()))
        .collect();
    for relation in ["=", "!=", "<", "<=", ">", ">="] {
        for sign in ["", "not ", "not not "] {
            cases.push((
                format!("{{p}}.q(N):-N=#count{{1:p}},{sign}N{relation}#count{{1:p}}."),
                format!(
                    "{{p}}.q(0):-0=#count{{1:p}},{sign}0{relation}#count{{1:p}}.q(1):-1=#count{{1:p}},{sign}1{relation}#count{{1:p}}."
                ),
            ));
        }
    }
    cases
}

const CASES: &[(&str, &str)] = &[
    (
        "p(1). n(N) :- N=#count{X:p(X)}, N<=#count{X:p(X)}.",
        "p(1).n(0):-0=#count{X:p(X)},0<=#count{X:p(X)}.n(1):-1=#count{X:p(X)},1<=#count{X:p(X)}.",
    ),
    (
        "{p}.q(N):-N=#count{1:p},N+1<=#count{1:p}<=N-1.",
        "{p}.q(0):-0=#count{1:p},1<=#count{1:p}<=-1.q(1):-1=#count{1:p},2<=#count{1:p}<=0.",
    ),
    (
        "{p}.q(N):-N=#count{1:p},not N+1<=#count{1:p}<=N-1.",
        "{p}.q(0):-0=#count{1:p},not 1<=#count{1:p}<=-1.q(1):-1=#count{1:p},not 2<=#count{1:p}<=0.",
    ),
    (
        "{p}.q(N,M):-N=#count{1:p},M=#sum{1:p},N<=#count{1:p}<=M.",
        "{p}.q(0,0):-0=#count{1:p},0=#sum{1:p},0<=#count{1:p}<=0.q(0,1):-0=#count{1:p},1=#sum{1:p},0<=#count{1:p}<=1.q(1,0):-1=#count{1:p},0=#sum{1:p},1<=#count{1:p}<=0.q(1,1):-1=#count{1:p},1=#sum{1:p},1<=#count{1:p}<=1.",
    ),
    (
        "r(N,M):-N=#count{},M=#count{},M<=#count{}.",
        "r(0,0):-0=#count{},0=#count{},0<=#count{}.",
    ),
    (
        "r(N,M):-N=#count{},M=#count{},M=#sum{}.",
        "r(0,0):-0=#count{},0=#count{},0=#sum{}.",
    ),
    (
        "q(M):-N=#count{},M=#sum{N},M=#count{}.",
        "q(0):-0=#count{},0=#sum{0},0=#count{}.",
    ),
    (
        "q(M):-N=#count{},M=#sum{N},M<=#count{}.",
        "q(0):-0=#count{},0=#sum{0},0<=#count{}.",
    ),
    (
        "{p}.q(N):-N=#count{1:p},N<=#count{1:p}<=N+1.",
        "{p}.q(0):-0=#count{1:p},0<=#count{1:p}<=1.q(1):-1=#count{1:p},1<=#count{1:p}<=2.",
    ),
    (
        "{p}.q(N):-not N<#count{1:p}<N+1,N=#count{1:p}.",
        "{p}.q(0):-not 0<#count{1:p}<1,0=#count{1:p}.q(1):-not 1<#count{1:p}<2,1=#count{1:p}.",
    ),
    (
        "{p}.q(Y):-#sum{2:p}>=Y,Y=N+1,N=#count{1:p}.",
        "{p}.q(1):-#sum{2:p}>=1,0=#count{1:p}.q(2):-#sum{2:p}>=2,1=#count{1:p}.",
    ),
    (
        "{p}.q(N):-N=#count{1:p},N>#sum{-2:p}.",
        "{p}.q(0):-0=#count{1:p},0>#sum{-2:p}.q(1):-1=#count{1:p},1>#sum{-2:p}.",
    ),
    (
        "{p}.q(N):-N=#count{1:p},N<=#sum+{-2:p;1:p;word:p}.",
        "{p}.q(0):-0=#count{1:p},0<=#sum+{-2:p;1:p;word:p}.q(1):-1=#count{1:p},1<=#sum+{-2:p;1:p;word:p}.",
    ),
    (
        "{p}.q(N):-N=#count{1:p},N<=#max{2:p}.",
        "{p}.q(0):-0=#count{1:p},0<=#max{2:p}.q(1):-1=#count{1:p},1<=#max{2:p}.",
    ),
    (
        "{p}.q(N):-N=#count{1:p},not not N>=#min{2:p}.",
        "{p}.q(0):-0=#count{1:p},not not 0>=#min{2:p}.q(1):-1=#count{1:p},not not 1>=#min{2:p}.",
    ),
    ("q:-N=#count{},N<=#min{}.", "q:-0=#count{},0<=#min{}."),
    (
        "{p}.q(N):-N=#count{1:p},N<=#count{N:p;0:p}.",
        "{p}.q(0):-0=#count{1:p},0<=#count{0:p;0:p}.q(1):-1=#count{1:p},1<=#count{1:p;0:p}.",
    ),
    (
        "{p}.q(N):-N=#count{1:p},not N=#count{1:p;1:not not p}.",
        "{p}.q(0):-0=#count{1:p},not 0=#count{1:p;1:not not p}.q(1):-1=#count{1:p},not 1=#count{1:p;1:not not p}.",
    ),
    (
        "p:-N=#count{},N!=#count{1:p}.",
        "p:-0=#count{},0!=#count{1:p}.",
    ),
    (
        "p:-N=#count{},not N=#count{1:p}.",
        "p:-0=#count{},not 0=#count{1:p}.",
    ),
    (
        "p:-N=#count{},not not N!=#count{1:p}.",
        "p:-0=#count{},not not 0!=#count{1:p}.",
    ),
    (
        "{p}.1{a;b}1:-N=#count{1:p},N<#count{1}.",
        "{p}.1{a;b}1:-0=#count{1:p},0<#count{1}.1{a;b}1:-1=#count{1:p},1<#count{1}.",
    ),
    (
        "{p}.1#count{1:a;2:b}1:-N=#count{1:p},not N=#count{}.",
        "{p}.1#count{1:a;2:b}1:-0=#count{1:p},not 0=#count{}.1#count{1:a;2:b}1:-1=#count{1:p},not 1=#count{}.",
    ),
    (
        "{p}.2#sum{-1:a;2:b}2:-N=#count{1:p},N>#count{}.",
        "{p}.2#sum{-1:a;2:b}2:-0=#count{1:p},0>#count{}.2#sum{-1:a;2:b}2:-1=#count{1:p},1>#count{}.",
    ),
    (
        "{p}.1#sum+{0:a;1:b}1:-N=#count{1:p},N>#count{}.",
        "{p}.1#sum+{0:a;1:b}1:-0=#count{1:p},0>#count{}.1#sum+{0:a;1:b}1:-1=#count{1:p},1>#count{}.",
    ),
    (
        "i(1..2).{p}.q(I,N):-i(I),N=#count{1:p},N<I,#count{I:p}>=N.",
        "i(1..2).{p}.q(I,0):-i(I),0=#count{1:p},0<I,#count{I:p}>=0.q(I,1):-i(I),1=#count{1:p},1<I,#count{I:p}>=1.",
    ),
    (
        "{p}.q(K):-K=1..N,N=#count{1:p},K<=#count{1:p}.",
        "{p}.q(K):-K=1..0,0=#count{1:p},K<=#count{1:p}.q(K):-K=1..1,1=#count{1:p},K<=#count{1:p}.",
    ),
    (
        "{p}.q(N):-N=#count{1:p},N<=#count{1:p},not N>#count{1:p}.",
        "{p}.q(0):-0=#count{1:p},0<=#count{1:p},not 0>#count{1:p}.q(1):-1=#count{1:p},1<=#count{1:p},not 1>#count{1:p}.",
    ),
];
