//! Explicit outer substitutions retain equality guards and scoped conditionals.

pub(super) const CASES: &[(&str, &str)] = &[
    ("q(N):-N=#count{};p(N):d.", "q(0):-0=#count{};p(0):d."),
    (
        "{d;p(0)}.q(N):-N=#count{};p(N):d.",
        "{d;p(0)}.q(0):-0=#count{};p(0):d.",
    ),
    (
        "{d;p(0)}.q(N):-N=#count{};not p(N):d.",
        "{d;p(0)}.q(0):-0=#count{};not p(0):d.",
    ),
    (
        "{d;p(0)}.q(N):-N=#count{};not not p(N):d.",
        "{d;p(0)}.q(0):-0=#count{};not not p(0):d.",
    ),
    (
        "{d;p(0)}.q(N):-N=#count{};p(N):not d.",
        "{d;p(0)}.q(0):-0=#count{};p(0):not d.",
    ),
    (
        "{d;p(0)}.q(N):-N=#count{};p(N):not not d.",
        "{d;p(0)}.q(0):-0=#count{};p(0):not not d.",
    ),
    (
        "{d(0);p}.q(N):-N=#count{};p:d(N).",
        "{d(0);p}.q(0):-0=#count{};p:d(0).",
    ),
    (
        "{d(0);p}.q(N):-N=#count{};p:not d(N).",
        "{d(0);p}.q(0):-0=#count{};p:not d(0).",
    ),
    (
        "{d(0);p}.q(N):-N=#count{};p:not not d(N).",
        "{d(0);p}.q(0):-0=#count{};p:not not d(0).",
    ),
    (
        "{d;p(1)}.q(Y):-N=#count{},Y=N+1;p(Y):d.",
        "{d;p(1)}.q(1):-0=#count{};p(1):d.",
    ),
    (
        "{d;p(1)}.q(Y):-N=#count{},Y=N+1;not p(Y):d.",
        "{d;p(1)}.q(1):-0=#count{};not p(1):d.",
    ),
    (
        "{d;p(f(0))}.q(N):-N=#count{};p(f(N)):d.",
        "{d;p(f(0))}.q(0):-0=#count{};p(f(0)):d.",
    ),
    (
        "{d;-p(0)}.q(N):-N=#count{};not -p(N):d.",
        "{d;-p(0)}.q(0):-0=#count{};not -p(0):d.",
    ),
    (
        "{d;p(f(0,a));p(f(0,b))}.q(N):-N=#count{};p(f(N,X)):d.",
        "{d;p(f(0,a));p(f(0,b))}.q(0):-0=#count{};p(f(0,X)):d.",
    ),
    (
        "{d;p(0,a);p(0,b)}.q(N):-N=#count{};p(N,_):d.",
        "{d;p(0,a);p(0,b)}.q(0):-0=#count{};p(0,_):d.",
    ),
    (
        "{p;s(0,a);s(0,b)}.q(N):-N=#count{};p:not s(N,_).",
        "{p;s(0,a);s(0,b)}.q(0):-0=#count{};p:not s(0,_).",
    ),
    (
        "{p;s(0,a);s(0,b)}.q(N):-N=#count{};p:not not s(N,_).",
        "{p;s(0,a);s(0,b)}.q(0):-0=#count{};p:not not s(0,_).",
    ),
    (
        "{p}.q(N):-N=#count{};p:not s(N,_).",
        "{p}.q(0):-0=#count{};p:not s(0,_).",
    ),
    ("{d}.q(N):-N=#count{};N=0:d.", "{d}.q(0):-0=#count{};0=0:d."),
    (
        "{d}.q(N):-N=#count{};not N=0:d.",
        "{d}.q(0):-0=#count{};not 0=0:d.",
    ),
    (
        "{d(0);p(0)}.q(N):-N=#count{};p(X):d(X),X=N.",
        "{d(0);p(0)}.q(0):-0=#count{};p(X):d(X),X=0.",
    ),
    (
        "{d;p(0);p(1)}.q(N):-N=#count{1:d};p(N):d.",
        "{d;p(0);p(1)}.q(0):-0=#count{1:d};p(0):d.q(1):-1=#count{1:d};p(1):d.",
    ),
    (
        "{d;p(-1)}.q(N):-N=#sum{-1:d};p(N):d.",
        "{d;p(-1)}.q(0):-0=#sum{-1:d};p(0):d.q(-1):--1=#sum{-1:d};p(-1):d.",
    ),
    (
        "{d;p(2)}.q(N):-N=#sum+{-1:d;2:d};p(N):d.",
        "{d;p(2)}.q(0):-0=#sum+{-1:d;2:d};p(0):d.q(2):-2=#sum+{-1:d;2:d};p(2):d.",
    ),
    (
        "{d;p(1)}.q(N):-N=#count{1:d};p(X):X=1..N.",
        "{d;p(1)}.q(0):-0=#count{1:d};p(X):X=1..0.q(1):-1=#count{1:d};p(X):X=1..1.",
    ),
    (
        "{d;p(1)}.q(M):-N=#count{1:d},M=#sum{N};p(M):d.",
        "{d;p(1)}.q(M):-0=#count{1:d},M=#sum{0};p(M):d.q(M):-1=#count{1:d},M=#sum{1};p(M):d.",
    ),
    (
        "{d;p(0)}.1{a;b}1:-N=#count{};not p(N):d.",
        "{d;p(0)}.1{a;b}1:-0=#count{};not p(0):d.",
    ),
    (
        "{d;p(0)}.1#count{1:a;2:b}1:-N=#count{};not not p(N):d.",
        "{d;p(0)}.1{a;b}1:-0=#count{};not not p(0):d.",
    ),
    (
        "{d}.p(N):-N=#count{};not not p(N):d.",
        "{d}.p(0):-0=#count{};not not p(0):d.",
    ),
    (
        "{d}.p(N):-N=#count{};p(N):d.",
        "{d}.p(0):-0=#count{};p(0):d.",
    ),
    (
        "{d;p(2)}.q(N):-N=#min{2:d};not p(N):d.",
        "{d;p(2)}.q(V):-V=#min{},#sup=#min{2:d};not p(V):d.q(2):-2=#min{2:d};not p(2):d.",
    ),
    (
        "{d;p(2)}.q(N):-N=#max{2:d};not not p(N):d.",
        "{d;p(2)}.q(V):-V=#max{},#inf=#max{2:d};not not p(V):d.q(2):-2=#max{2:d};not not p(2):d.",
    ),
    (
        "d(a).e(b).{p(0,a);r(0,b)}.q(N):-N=#count{};p(N,X):d(X);r(N,X):e(X).",
        "d(a).e(b).{p(0,a);r(0,b)}.q(0):-0=#count{};p(0,X):d(X);r(0,X):e(X).",
    ),
];
