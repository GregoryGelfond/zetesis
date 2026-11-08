% 2QBF by saturation: is there an assignment of the existential variables
% x(1..exists) under which a formula in disjunctive normal form holds for every
% assignment of the universal variables y(1..forall)? Each answer set is one such
% existential assignment; the program is unsatisfiable when there is none.
%
% The formula has `terms` terms. Each term has one existential literal and
% `width` universal literals over distinct variables, so whether a term holds
% always depends on the universal assignment. A fixed hash of `seed`, the term
% and the variable chooses every literal, so the constants alone determine the
% instance. The hash squares modulo the prime 46337, so every intermediate value
% stays below 2^31.

#const exists = 3.
#const forall = 8.
#const width = 3.
#const terms = 68.
#const seed = 1.

term(1..terms).

% One hashed key per term and variable slot: slot 0 for the existential literal,
% slots 1..forall for the universal variables.
key(T, J, K) :-
    term(T), J = 0..forall,
    H0 = (seed * 7919 + T * 104729 + J * 1299709) \ 46337,
    H1 = (H0 * H0 + 12345) \ 46337,
    H2 = (H1 * H1 + 54321) \ 46337,
    K = (H2 * H2 + 7) \ 46337.

% A term's existential literal, and its universal literals: the `width`
% variables of smallest rank, the key with the variable's index appended, so
% that ranks are distinct while `forall` stays below 32. The key's higher bits
% give each literal's sign.
rank(T, J, K * 32 + J) :- key(T, J, K), J > 0.
existential(T, K \ exists + 1, (K / 61) \ 2) :- key(T, 0, K).
universal(T, J, (K / 61) \ 2) :-
    key(T, J, K), rank(T, J, R),
    #count { L : rank(T, L, M), M < R } < width.

% The existential guess, and the universal guess.
x(I) | nx(I) :- I = 1..exists.
y(J) | ny(J) :- J = 1..forall.

% A literal holds when its variable takes its sign; a term holds when all its
% literals hold; the formula holds when one of its terms does.
active(T) :- existential(T, I, 1), x(I).
active(T) :- existential(T, I, 0), nx(I).
holds(T, J) :- universal(T, J, 1), y(J).
holds(T, J) :- universal(T, J, 0), ny(J).
valid :- active(T), holds(T, J) : universal(T, J, _).

% Saturation: once the formula holds, every universal atom holds too. An answer
% set must therefore be saturated, and it is minimal exactly when no universal
% assignment falsifies the formula under its existential assignment.
y(J) :- J = 1..forall, valid.
ny(J) :- J = 1..forall, valid.
:- not valid.

#show x/1.
#show nx/1.
