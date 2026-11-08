% 2QBF by saturation over the pigeonhole principle: is there an assignment of the
% existential variables x(1..exists) under which every assignment of the
% universal variables falsifies the core the existential assignment selects?
% Each answer set is one such existential assignment.
%
% The core places holes + 1 pigeons into `holes` holes: each pigeon sits in some
% hole, and no two pigeons share one, so no assignment satisfies it. Each
% existential assignment A = 0..2^exists - 1 selects a core: the first `stable`
% assignments get the whole contradiction, and every other assignment gets it
% less one clause, which makes it satisfiable, since every clause of the
% pigeonhole formula is needed for its contradiction. The answer sets are
% therefore exactly the first `stable` existential assignments. Unlike the parity
% cores, the satisfiable cores here are also hard to search, so every candidate's
% minimality check carries real work.

#const exists = 2.
#const stable = 1.
#const holes = 4.

% The existential assignments and their bits: bit(A, I, B) when variable I is B
% in assignment A.
power(1, 1).
power(I + 1, 2 * P) :- power(I, P), I < exists.
assignment(0..2 * P - 1) :- power(exists, P).
bit(A, I, (A / P) \ 2) :- assignment(A), power(I, P).

% One universal variable per pigeon and hole: var(I, J, V) when V says pigeon I
% sits in hole J.
pigeon(1..holes + 1).
hole(1..holes).
var(I, J, (I - 1) * holes + J) :- pigeon(I), hole(J).
universal(V) :- var(_, _, V).

% Clause I says pigeon I sits in some hole; each later clause says two pigeons do
% not share a hole: literal(C, V, 1) is positive, literal(C, V, 0) negative.
literal(I, V, 1) :- var(I, J, V).
literal(C, V, 0) :-
    var(I, J, V), var(K, J, _), I < K,
    C = holes + 1 + ((J - 1) * (holes + 1) + I - 1) * (holes + 1) + K.
literal(C, V, 0) :-
    var(I, J, _), var(K, J, V), I < K,
    C = holes + 1 + ((J - 1) * (holes + 1) + I - 1) * (holes + 1) + K.
clause(C) :- literal(C, _, _).

% Each assignment beyond the first `stable` drops one clause from its core: the
% clause of rank A - stable + 1.
ordinal(C, R) :- clause(C), R = #count { D : clause(D), D <= C }.
dropped(A, C) :- assignment(A), A >= stable, ordinal(C, A - stable + 1).
active(A, C) :- assignment(A), clause(C), not dropped(A, C).

% The existential guess selects one assignment.
x(I) | nx(I) :- I = 1..exists.
off(A) :- bit(A, I, 1), nx(I).
off(A) :- bit(A, I, 0), x(I).
current(A) :- assignment(A), not off(A).

% The universal guess. A clause is falsified when each of its literals is, and
% the formula holds when the current core has a falsified clause.
y(J) | ny(J) :- universal(J).
falsified(C, J) :- literal(C, J, 1), ny(J).
falsified(C, J) :- literal(C, J, 0), y(J).
falsified(C) :- clause(C), falsified(C, J) : literal(C, J, _).
valid :- current(A), active(A, C), falsified(C).

% Saturation: once the formula holds, every universal atom holds too. An answer
% set must therefore be saturated, and it is minimal exactly when no universal
% assignment satisfies its core.
y(J) :- universal(J), valid.
ny(J) :- universal(J), valid.
:- not valid.

#show x/1.
#show nx/1.
