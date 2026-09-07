% Scenario 03: A subset's only vertex has no incoming edge (UNSAT).
% @expect unsat
%
% Vertices s (start), a (subset 1), b (subset 2). Return arcs a -> s and b -> s
% exist, but nothing reaches b. Subset 2 forces b onto the tour, yet b has no
% incoming tour edge, so the program is UNSAT.

#include "../../encodings/equality-generalized-tsp/egtsp.lp".

vertex(s; a; b).

edge(
    s,a,1;
    a,s,1;
    b,s,1
).

start(s).

in_subset(
    a,1;
    b,2
).
