% 2QBF by saturation over a parity contradiction: is there an assignment of the
% existential variables x(1..exists) under which every assignment of the
% universal variables falsifies the core the existential assignment selects?
% Each answer set is one such existential assignment.
%
% The core is a parity contradiction over n inputs. Two chains of three-variable
% exclusive-or gates compute the inputs' parity, the second in a hashed order,
% and the two results are required to differ, so no assignment of the inputs and
% the chains' auxiliary variables satisfies it. Each existential assignment
% A = 0..2^exists - 1 selects a core: the first `stable` assignments get the
% whole contradiction, and every other assignment gets it less one clause, which
% makes it satisfiable. The answer sets are therefore exactly the first `stable`
% existential assignments, and each one's minimality check must refute the
% whole contradiction over every universal assignment.
%
% The hash squares modulo the prime 46337, so every intermediate value stays
% below 2^31.

#const exists = 2.
#const stable = 1.
#const n = 6.
#const seed = 1.

% The existential assignments and their bits: bit(A, I, B) when variable I is B
% in assignment A.
power(1, 1).
power(I + 1, 2 * P) :- power(I, P), I < exists.
assignment(0..2 * P - 1) :- power(exists, P).
bit(A, I, (A / P) \ 2) :- assignment(A), power(I, P).

% Universal variables: the inputs 1..n, the first chain's outputs n+1..2n-1 and
% the second chain's outputs 2n..3n-2.
universal(1..3 * n - 2).

% The second chain's order: the inputs ranked by a hash of the seed, with the
% input's index appended so that ranks are distinct while n stays below 32.
key(J, K) :-
    J = 1..n,
    H0 = (seed * 7919 + J * 104729) \ 46337,
    H1 = (H0 * H0 + 12345) \ 46337,
    H2 = (H1 * H1 + 54321) \ 46337,
    K = (H2 * H2 + 7) \ 46337.
rank(J, K * 32 + J) :- key(J, K).
order(P, J) :- rank(J, R), P = #count { L : rank(L, S), S <= R }.

% Gates gate(Chain, K, Acc, In, Out): Out is Acc xor In.
gate(1, 1, 1, 2, n + 1).
gate(1, K, n + K - 1, K + 1, n + K) :- K = 2..n - 1.
gate(2, 1, A, B, 2 * n) :- order(1, A), order(2, B).
gate(2, K, 2 * n + K - 2, B, 2 * n - 1 + K) :- K = 2..n - 1, order(K + 1, B).

% A gate forbids the four patterns of odd parity, one clause each. A clause
% forbidding a pattern has, on each variable, the literal its value falsifies:
% literal(C, J, 1) is positive, literal(C, J, 0) negative.
odd(1, 1, 0, 0).
odd(2, 0, 1, 0).
odd(3, 0, 0, 1).
odd(4, 1, 1, 1).
literal(C, Acc, 1 - S1) :-
    gate(Chain, K, Acc, In, Out), odd(P, S1, S2, S3),
    C = ((Chain - 1) * (n - 1) + K - 1) * 4 + P.
literal(C, In, 1 - S2) :-
    gate(Chain, K, Acc, In, Out), odd(P, S1, S2, S3),
    C = ((Chain - 1) * (n - 1) + K - 1) * 4 + P.
literal(C, Out, 1 - S3) :-
    gate(Chain, K, Acc, In, Out), odd(P, S1, S2, S3),
    C = ((Chain - 1) * (n - 1) + K - 1) * 4 + P.

% The two chains' results differ: neither both false nor both true.
literal(8 * (n - 1) + 1, 2 * n - 1, 1).
literal(8 * (n - 1) + 1, 3 * n - 2, 1).
literal(8 * (n - 1) + 2, 2 * n - 1, 0).
literal(8 * (n - 1) + 2, 3 * n - 2, 0).
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
