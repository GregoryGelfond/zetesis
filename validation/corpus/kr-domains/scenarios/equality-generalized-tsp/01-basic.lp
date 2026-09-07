% Scenario 01: Five vertices, two subsets, a clear cheapest representative each.
% @expect sat
% @cost { 3 }
% @optimal { included(a,c,1), included(c,s,1), included(s,a,1), visited(a), visited(c), visited(s), start(s) }
%
% Vertices s (start), a and b (subset 1), c and d (subset 2). Cheap arcs run
% s -> a -> c -> s; the alternatives are uniformly expensive. The optimum picks a
% and c, taking s -> a -> c -> s at cost 3.

#include "../../encodings/equality-generalized-tsp/egtsp.lp".

vertex(s; a; b; c; d).

edge(
    s,a,1;
    s,b,5;
    a,c,1;
    a,d,5;
    b,c,5;
    b,d,5;
    c,s,1;
    d,s,5
).

start(s).

in_subset(
    a,1;
    b,1;
    c,2;
    d,2
).
