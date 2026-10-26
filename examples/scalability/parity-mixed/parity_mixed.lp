% A satisfiable mixture of parity and ordinary clauses: Tseitin's parity
% formula with every vertex charged 0 on a connected 3-regular graph, whose
% solutions are the graph's even subgraphs, cut down by `clauses` random
% three-literal clauses over its edges. The parity constraints are structured;
% the random clauses are not, so a solver must still search among the even
% subgraphs for those the clauses allow. The answer sets are those subgraphs.
%
% The graph is the Tseitin workload's: 2p vertices, 0..2p - 1, a cycle through
% them all and a perfect matching that joins each vertex I < p to vertex
% p + pi(I), with pi(I) = (I + seed)^3 + 7 mod p, a bijection when p is a prime
% whose remainder modulo 3 is not 1 (2, 3, 5, 11, 17, 23, 29, ...). A fixed
% hash of `seed`, the clause and the literal chooses every literal. It squares
% modulo the prime 46337, so every intermediate value stays below 2^31.

#const p = 5.
#const clauses = 8.
#const seed = 1.

vertex(0..2 * p - 1).

% edge(E, U, V): edge E joins vertices U and V. The cycle's edges are 1..2p,
% the matching's 2p + 1..3p.
edge(U + 1, U, (U + 1) \ (2 * p)) :- vertex(U).
edge(2 * p + 1 + I, I, p + M) :-
    I = 0..p - 1,
    U = (I + seed) \ p,
    S = (U * U) \ p,
    M = (S * U + 7) \ p.

% A vertex's three edges, in increasing order.
incident(V, E) :- edge(E, V, _).
incident(V, E) :- edge(E, _, V).
edges(V, A, B, C) :-
    incident(V, A), incident(V, B), incident(V, C), A < B, B < C.

% Each edge is in the subgraph or not.
{ on(E) } :- edge(E, _, _).

% Every vertex has charge 0: it forbids an odd number of its edges on.
:- edges(V, A, B, C), on(A), not on(B), not on(C).
:- edges(V, A, B, C), not on(A), on(B), not on(C).
:- edges(V, A, B, C), not on(A), not on(B), on(C).
:- edges(V, A, B, C), on(A), on(B), on(C).

% clause(K, A, SA, B, SB, C, SC): the K-th clause holds when on(A) has the
% value SA, on(B) has SB or on(C) has SC. One hashed key per clause and
% literal; the keys choose three distinct edges, as a first edge and two
% positive steps around the edges that together stay short of a full turn, and
% each literal's sign.
key(K, J, X) :-
    K = 1..clauses, J = 1..3,
    H0 = (seed * 7919 + K * 104729 + J * 1299709) \ 46337,
    H1 = (H0 * H0 + 12345) \ 46337,
    H2 = (H1 * H1 + 54321) \ 46337,
    X = (H2 * H2 + 7) \ 46337.
clause(K, A + 1, (X1 / 61) \ 2, (A + D) \ (3 * p) + 1, (X2 / 61) \ 2,
       (A + D + F) \ (3 * p) + 1, (X3 / 61) \ 2) :-
    key(K, 1, X1), key(K, 2, X2), key(K, 3, X3),
    A = X1 \ (3 * p),
    D = 1 + X2 \ (3 * p - 2),
    F = 1 + X3 \ (3 * p - 1 - D).

% A clause forbids the one pattern that falsifies its three literals.
:- clause(K, A, 0, B, 0, C, 0), on(A), on(B), on(C).
:- clause(K, A, 0, B, 0, C, 1), on(A), on(B), not on(C).
:- clause(K, A, 0, B, 1, C, 0), on(A), not on(B), on(C).
:- clause(K, A, 0, B, 1, C, 1), on(A), not on(B), not on(C).
:- clause(K, A, 1, B, 0, C, 0), not on(A), on(B), on(C).
:- clause(K, A, 1, B, 0, C, 1), not on(A), on(B), not on(C).
:- clause(K, A, 1, B, 1, C, 0), not on(A), not on(B), on(C).
:- clause(K, A, 1, B, 1, C, 1), not on(A), not on(B), not on(C).

#show on/1.
