% 2QBF by saturation over a Tseitin contradiction: is there an assignment of the
% existential variables x(1..exists) under which every assignment of the
% universal variables falsifies the core the existential assignment selects?
% Each answer set is one such existential assignment.
%
% The core is Tseitin's parity formula on a connected 3-regular graph: one
% universal variable per edge, and at each vertex the exclusive-or of its three
% edges equals the vertex's charge. Vertex 1 has charge 1 and every other vertex
% charge 0, so the charges' sum is odd and no assignment of the edges satisfies
% the formula. Each existential assignment A = 0..2^exists - 1 selects a core:
% the first `stable` assignments get the whole contradiction, and every other
% assignment gets it less one clause of vertex 1 or 2, which makes it
% satisfiable, since removing either vertex leaves each graph connected. The
% answer sets are therefore exactly the first `stable` existential assignments.
%
% The graphs are fixed: random 3-regular graphs on 12, 18, 24 and 30 vertices,
% committed below. `vertices` selects one.

#const exists = 2.
#const stable = 1.
#const vertices = 12.

% The existential assignments and their bits: bit(A, I, B) when variable I is B
% in assignment A.
power(1, 1).
power(I + 1, 2 * P) :- power(I, P), I < exists.
assignment(0..2 * P - 1) :- power(exists, P).
bit(A, I, (A / P) \ 2) :- assignment(A), power(I, P).

% edge(N, E, U, V): edge E joins vertices U and V in the graph on N vertices.
edge(12,1,1,3). edge(12,2,1,5). edge(12,3,1,7). edge(12,4,2,4). edge(12,5,2,6). edge(12,6,2,11).
edge(12,7,3,9). edge(12,8,3,11). edge(12,9,4,7). edge(12,10,4,8). edge(12,11,5,10). edge(12,12,5,11).
edge(12,13,6,8). edge(12,14,6,9). edge(12,15,7,12). edge(12,16,8,12). edge(12,17,9,10). edge(12,18,10,12).

edge(18,1,1,9). edge(18,2,1,15). edge(18,3,1,16). edge(18,4,2,4). edge(18,5,2,11). edge(18,6,2,12).
edge(18,7,3,9). edge(18,8,3,10). edge(18,9,3,13). edge(18,10,4,10). edge(18,11,4,11). edge(18,12,5,8).
edge(18,13,5,13). edge(18,14,5,15). edge(18,15,6,10). edge(18,16,6,15). edge(18,17,6,17). edge(18,18,7,14).
edge(18,19,7,16). edge(18,20,7,18). edge(18,21,8,9). edge(18,22,8,12). edge(18,23,11,18). edge(18,24,12,16).
edge(18,25,13,14). edge(18,26,14,17). edge(18,27,17,18).

edge(24,1,1,3). edge(24,2,1,5). edge(24,3,1,24). edge(24,4,2,7). edge(24,5,2,14). edge(24,6,2,24).
edge(24,7,3,12). edge(24,8,3,19). edge(24,9,4,6). edge(24,10,4,16). edge(24,11,4,22). edge(24,12,5,11).
edge(24,13,5,15). edge(24,14,6,8). edge(24,15,6,13). edge(24,16,7,11). edge(24,17,7,18). edge(24,18,8,10).
edge(24,19,8,17). edge(24,20,9,11). edge(24,21,9,23). edge(24,22,9,24). edge(24,23,10,14). edge(24,24,10,17).
edge(24,25,12,19). edge(24,26,12,23). edge(24,27,13,15). edge(24,28,13,23). edge(24,29,14,21). edge(24,30,15,19).
edge(24,31,16,17). edge(24,32,16,20). edge(24,33,18,20). edge(24,34,18,21). edge(24,35,20,22). edge(24,36,21,22).

edge(30,1,1,3). edge(30,2,1,19). edge(30,3,1,27). edge(30,4,2,9). edge(30,5,2,10). edge(30,6,2,15).
edge(30,7,3,15). edge(30,8,3,24). edge(30,9,4,7). edge(30,10,4,10). edge(30,11,4,29). edge(30,12,5,12).
edge(30,13,5,16). edge(30,14,5,28). edge(30,15,6,8). edge(30,16,6,18). edge(30,17,6,28). edge(30,18,7,20).
edge(30,19,7,29). edge(30,20,8,20). edge(30,21,8,25). edge(30,22,9,16). edge(30,23,9,21). edge(30,24,10,13).
edge(30,25,11,19). edge(30,26,11,24). edge(30,27,11,25). edge(30,28,12,22). edge(30,29,12,30). edge(30,30,13,17).
edge(30,31,13,26). edge(30,32,14,22). edge(30,33,14,27). edge(30,34,14,30). edge(30,35,15,23). edge(30,36,16,18).
edge(30,37,17,23). edge(30,38,17,29). edge(30,39,18,25). edge(30,40,19,30). edge(30,41,20,22). edge(30,42,21,23).
edge(30,43,21,28). edge(30,44,24,26). edge(30,45,26,27).

% The selected graph's edges are the universal variables. slot(V, E, K) when E
% is vertex V's K-th incident edge in edge order.
universal(E) :- edge(vertices, E, _, _).
incident(V, E) :- edge(vertices, E, V, _).
incident(V, E) :- edge(vertices, E, _, V).
slot(V, E, K) :- incident(V, E), K = #count { D : incident(V, D), D <= E }.
charge(1, 1).
charge(V, 0) :- incident(V, _), V > 1.

% A vertex forbids each pattern of its three edges whose parity differs from its
% charge, one clause each: clause (V - 1) * 4 + I for the I-th such pattern. A
% clause forbidding a pattern has, on each edge, the literal its value
% falsifies: literal(C, E, 1) is positive, literal(C, E, 0) negative.
pattern(4 * S1 + 2 * S2 + S3 + 1, S1, S2, S3) :- S1 = 0..1, S2 = 0..1, S3 = 0..1.
forbidden(V, P) :- charge(V, G), pattern(P, S1, S2, S3), (S1 + S2 + S3 + G) \ 2 = 1.
index(V, P, I) :- forbidden(V, P), I = #count { Q : forbidden(V, Q), Q <= P }.
literal((V - 1) * 4 + I, E, 1 - S1) :- index(V, P, I), pattern(P, S1, S2, S3), slot(V, E, 1).
literal((V - 1) * 4 + I, E, 1 - S2) :- index(V, P, I), pattern(P, S1, S2, S3), slot(V, E, 2).
literal((V - 1) * 4 + I, E, 1 - S3) :- index(V, P, I), pattern(P, S1, S2, S3), slot(V, E, 3).
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
