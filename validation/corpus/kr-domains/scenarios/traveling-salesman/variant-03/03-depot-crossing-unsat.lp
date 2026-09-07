% Scenario 03: Customer reachable only by crossing the other depot (UNSAT).
% @expect unsat
%
% A directed cycle a -> b -> c -> a with depots at a and b. For s1 (depot a) to
% visit c it must run a -> b -> c -> a, passing through b, which is s2's depot;
% the one-salesman-per-vertex count forbids that. Symmetrically s2 can only return
% to b through a. A salesman skipping its tour fails returns-to-depot. So UNSAT.
% With a single shared start (variant-02) the same graph has a valid one-tour
% solution at cost 3; variant-03's per-salesman depots are what make it infeasible.

#include "../../../encodings/traveling-salesman/variant-03.lp".

vertex(a; b; c).

edge(
    a,b,1;
    b,c,1;
    c,a,1
).

salesman(s1; s2).
depot(
    s1,a;
    s2,b
).
