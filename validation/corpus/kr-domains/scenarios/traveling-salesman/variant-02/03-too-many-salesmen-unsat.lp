% Scenario 03: Two salesmen, only one Hamiltonian cycle (UNSAT).
% @expect unsat
%
% The graph has exactly one tour through the start, a -> b -> c -> a. A second
% salesman would need a disjoint tour (none exists) or would share a vertex with
% the first (forbidden by the one-salesman-per-non-start-vertex constraint), and a
% salesman that traverses nothing fails returns-to-start. So the program is UNSAT.

#include "../../../encodings/traveling-salesman/variant-02.lp".

vertex(a; b; c).

edge(
    a,b,1;
    b,c,1;
    c,a,1
).

start(a).

salesman(s1; s2).
