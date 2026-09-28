//! Authored finite substitutions retain original aggregate equalities.

// All carriers are deliberately tiny so every original/frozen interpretation
// pair can be checked. Correlated independent aggregates include unrealizable
// proposal pairs: arithmetic success cannot establish either equality.
pub(super) const CASES: &[(&str, &str)] = &[
    ("Y{p}:-N=#count{},Y=N+1.", "1{p}:-0=#count{}."),
    ("{p}:-N=#count{},N>0.", ""),
    (
        "{d}.Y{a;b}Y:-N=#count{1:d},Y=N+1.",
        "{d}.1{a;b}1:-0=#count{1:d}.2{a;b}2:-1=#count{1:d}.",
    ),
    (
        "{d}.Y{a;b}Y:-Y=N+1,N=#count{1:d}.",
        "{d}.1{a;b}1:-0=#count{1:d}.2{a;b}2:-1=#count{1:d}.",
    ),
    (
        "{d}.Y#count{1:a;2:b}Y:-N=#count{1:d},Y=N+1.",
        "{d}.1{a;b}1:-0=#count{1:d}.2{a;b}2:-1=#count{1:d}.",
    ),
    (
        "{d}.Y#count{1:a;2:b}Y:-Y=N+1,N=#count{1:d}.",
        "{d}.1{a;b}1:-0=#count{1:d}.2{a;b}2:-1=#count{1:d}.",
    ),
    (
        "{d}.Y{a;b}Y:-N=#sum{-1:d},Y=N+1.",
        "{d}.1{a;b}1:-0=#sum{-1:d}.0{a;b}0:--1=#sum{-1:d}.",
    ),
    (
        "{d}.Y#count{1:a;2:b}Y:-N=#sum+{-1:d;2:d},Y=N+1.",
        "{d}.1{a;b}1:-0=#sum+{-1:d;2:d}.3{a;b}3:-2=#sum+{-1:d;2:d}.",
    ),
    ("{d}.1{a}1:-N=#min{2:d},N=2.", "{d}.1{a}1:-2=#min{2:d}."),
    (
        "{d}.1#count{1:a}1:-N=#max{2:d},N=2.",
        "{d}.1{a}1:-2=#max{2:d}.",
    ),
    (
        "{d}.L{a;b}U:-N=#count{1:d},M=#sum{2:d},L=N+1,U=M+1.",
        "{d}.1{a;b}1:-0=#count{1:d},0=#sum{2:d}.1{a;b}3:-0=#count{1:d},2=#sum{2:d}.2{a;b}1:-1=#count{1:d},0=#sum{2:d}.2{a;b}3:-1=#count{1:d},2=#sum{2:d}.",
    ),
    (
        "{d}.L#count{1:a;2:b}U:-U=M+1,L=N+1,M=#sum{2:d},N=#count{1:d}.",
        "{d}.1{a;b}1:-0=#count{1:d},0=#sum{2:d}.1{a;b}3:-0=#count{1:d},2=#sum{2:d}.2{a;b}1:-1=#count{1:d},0=#sum{2:d}.2{a;b}3:-1=#count{1:d},2=#sum{2:d}.",
    ),
    (
        "{d}.1{a;b}1:-N=#count{1:d},N>0.",
        "{d}.1{a;b}1:-1=#count{1:d}.",
    ),
    (
        "{d}.1#count{1:a;2:b}1:-N=#count{1:d},(N,1)=(1,1).",
        "{d}.1{a;b}1:-1=#count{1:d}.",
    ),
    (
        "{d}.1{a;b}1:-N=#count{1:d},not N=0.",
        "{d}.1{a;b}1:-1=#count{1:d}.",
    ),
    (
        "{d}.1#count{1:a;2:b}1:-N=#count{1:d},not not 0<N<=1.",
        "{d}.1{a;b}1:-1=#count{1:d}.",
    ),
    (
        "{d;e}.Y{a:b;b}Y:-e,N=#count{1:d},Y=N+1.",
        "{d;e}.1{a:b;b}1:-e,0=#count{1:d}.2{a:b;b}2:-e,1=#count{1:d}.",
    ),
    (
        "{d;e}.Y#count{1:a:b;2:b}Y:-Y=N+1,N=#count{1:d},e.",
        "{d;e}.1{a:b;b}1:-e,0=#count{1:d}.2{a:b;b}2:-e,1=#count{1:d}.",
    ),
    (
        "{p(1);p(2)}.1{a}1:-N=#count{1:p(1)},p(N+1).",
        "{p(1);p(2)}.1{a}1:-0=#count{1:p(1)},p(1).1{a}1:-1=#count{1:p(1)},p(2).",
    ),
    (
        "{p(f(1));p(f(2))}.1#count{1:a}1:-p(f(Y+1)),Y=N,N=#count{1:p(f(1))}.",
        "{p(f(1));p(f(2))}.1{a}1:-0=#count{1:p(f(1))},p(f(1)).1{a}1:-1=#count{1:p(f(1))},p(f(2)).",
    ),
    (
        "{-p(1);-p(2)}.1{a}1:-N=#count{1: -p(1)},-p(N+1).",
        "{-p(1);-p(2)}.1{a}1:-0=#count{1: -p(1)},-p(1).1{a}1:-1=#count{1: -p(1)},-p(2).",
    ),
    ("Y{}Y:-N=#count{},Y=N+1.", "1{}1:-0=#count{}."),
    ("Y#count{}Y:-N=#count{},Y=N+1.", "1{}1:-0=#count{}."),
    ("Y{}Y:-N=#count{},Y=N+1,N>0.", ""),
    ("Y#count{}Y:-N=#count{},Y=N+1,N>0.", ""),
    (
        "{d}.Y{p(N)}Y:-N=#count{1:d},Y=N+1.",
        "{d}.1{p(0)}1:-0=#count{1:d}.2{p(1)}2:-1=#count{1:d}.",
    ),
    (
        "{d}.Y#count{N:p(N)}Y:-N=#count{1:d},Y=N+1.",
        "{d}.1{p(0)}1:-0=#count{1:d}.2{p(1)}2:-1=#count{1:d}.",
    ),
    (
        "{d}.1#count{1:p(Y)}1:-N=#count{1:d},Y=N+1.",
        "{d}.1{p(1)}1:-0=#count{1:d}.1{p(2)}1:-1=#count{1:d}.",
    ),
    (
        "{d}.1#count{X:p(X):X=Y..Y}1:-N=#count{1:d},Y=N+1.",
        "{d}.1{p(1)}1:-0=#count{1:d}.1{p(2)}1:-1=#count{1:d}.",
    ),
    ("Y{a:a}Y:-N=#count{},Y=N+1.", "1{a:a}1:-0=#count{}."),
    ("Y#count{1:a:a}Y:-N=#count{},Y=N+1.", "1{a:a}1:-0=#count{}."),
    (
        "{d}.Y#count{1:a:a;1:a:b;2:b}Y:-N=#count{1:d},Y=N+1.",
        "{d}.1{a:a;a:b;b}1:-0=#count{1:d}.2{a:a;a:b;b}2:-1=#count{1:d}.",
    ),
];
