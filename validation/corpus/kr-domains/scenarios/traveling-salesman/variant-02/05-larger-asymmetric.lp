% Scenario 05: Larger 5-vertex instance, two cycles of different size.
% @expect sat
% @cost { 7 }
% @count optimal 2
%
% Two cycles share only the start: a -> b -> a (cost 2) and a -> c -> d -> e -> a
% (cost 5). A single salesman leaves the start once, so it can enter only one
% cycle; the two salesmen split them, for a total of 7, with two co-optimal
% assignments.

#include "../../../encodings/traveling-salesman/variant-02.lp".

vertex(a; b; c; d; e).

% Small cycle: a-b-a.
edge(
    a,b,1;
    b,a,1
).

% Larger cycle: a-c-d-e-a.
edge(
    a,c,2;
    c,d,1;
    d,e,1;
    e,a,1
).

start(a).

salesman(s1; s2).
