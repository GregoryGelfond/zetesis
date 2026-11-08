% Tseitin's parity formula on a connected 3-regular graph. Each edge is a free
% choice, and at each vertex the exclusive-or of its three edges must equal the
% vertex's charge. Vertex 0 has charge `odd` and every other vertex charge 0.
% With `odd = 1` the charges' sum is odd, so no choice of edges satisfies every
% vertex: the program is unsatisfiable. With `odd = 0` the solutions are the
% graph's even subgraphs, 2^(p + 1) answer sets.
%
% Each vertex states its parity as one integrity constraint per forbidden
% pattern of its three edges, the formula's clauses. Resolution, and so a
% solver that learns clauses, needs exponentially many steps to refute these
% constraints on an expanding graph; elimination over GF(2) refutes them in
% polynomial time.
%
% The graph has 2p vertices, 0..2p - 1: a cycle through them all, which makes
% it connected, and a perfect matching that joins each vertex I < p to vertex
% p + pi(I). The permutation pi of 0..p - 1 is I -> (I + seed)^3 + 7 mod p. It
% is a bijection when p is a prime whose remainder modulo 3 is not 1 (2, 3, 5,
% 11, 17, 23, 29, ...), because cubing permutes the residues modulo such a
% prime. Every intermediate value stays below p^2.

#const p = 5.
#const seed = 1.
#const odd = 1.

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

charge(0, odd).
charge(V, 0) :- vertex(V), V > 0.

% Each edge is in the subgraph or not.
{ on(E) } :- edge(E, _, _).

% A vertex of charge 0 forbids an odd number of its edges on.
:- edges(V, A, B, C), charge(V, 0), on(A), not on(B), not on(C).
:- edges(V, A, B, C), charge(V, 0), not on(A), on(B), not on(C).
:- edges(V, A, B, C), charge(V, 0), not on(A), not on(B), on(C).
:- edges(V, A, B, C), charge(V, 0), on(A), on(B), on(C).

% A vertex of charge 1 forbids an even number.
:- edges(V, A, B, C), charge(V, 1), not on(A), not on(B), not on(C).
:- edges(V, A, B, C), charge(V, 1), on(A), on(B), not on(C).
:- edges(V, A, B, C), charge(V, 1), on(A), not on(B), on(C).
:- edges(V, A, B, C), charge(V, 1), not on(A), on(B), on(C).

#show on/1.
