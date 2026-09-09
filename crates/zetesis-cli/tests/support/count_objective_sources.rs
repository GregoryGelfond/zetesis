//! Original sources shared by prepared CPU and physical-device qualification.

/// Consistent independent count-head and forwarded-objective programs.
pub const SATISFIABLE: [&str; 4] = [
    "1#count{1:a;1:b}1.{d}.n(N):-N=#count{1:d}.p(X):-n(X).#minimize{X@7:p(X)}.",
    "2#count{1:a;2:a}2.{d}.n(N):-N=#count{1:d}.p(X):-n(X).#minimize{X@7:p(X)}.",
    "0#count{1:a;2:a}2.{d}.n(N):-N=#count{1:d;2:d}.p(X):-n(X).#maximize{X@7:p(X)}.",
    "{b;d}.1#count{1:a;1:c}1:-b.n(N):-N=#count{1:d}.p(X):-n(X).#minimize{X@7:p(X)}.",
];

/// Two tuples share one atom, so exactly one active tuple is impossible.
pub const INCONSISTENT: &str =
    "1#count{1:a;2:a}1.{d}.n(N):-N=#count{1:d}.p(X):-n(X).#minimize{X@7:p(X)}.";
