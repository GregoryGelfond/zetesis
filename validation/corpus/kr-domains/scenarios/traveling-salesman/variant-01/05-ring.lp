% Scenario 05: 5-vertex ring with diagonal shortcuts.
% @expect sat
% @cost { 5 }
% @count optimal 2
%
% A ring a-b-c-d-e-a with both directions at cost 1, plus full diagonals at cost
% 5. The cheap Hamiltonian cycle is the ring itself, cost 5 (five unit edges); any
% tour taking a diagonal swaps a cost-5 edge for two cost-1 edges and so must add
% a unit edge elsewhere, exceeding 5. The two ring directions are co-optimal.

#include "../../../encodings/traveling-salesman/variant-01.lp".

vertex(a; b; c; d; e).

% Ring edges (both directions, cost 1).
edge(
    a,b,1;
    b,a,1;
    b,c,1;
    c,b,1;
    c,d,1;
    d,c,1;
    d,e,1;
    e,d,1;
    e,a,1;
    a,e,1
).

% Diagonal shortcuts (both directions, cost 5).
edge(
    a,c,5;
    c,a,5;
    a,d,5;
    d,a,5;
    b,d,5;
    d,b,5;
    b,e,5;
    e,b,5;
    c,e,5;
    e,c,5
).

start(a).
