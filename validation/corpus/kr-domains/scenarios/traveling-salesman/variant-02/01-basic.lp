% Scenario 01: Basic 2-salesman split on a 4-vertex graph.
% @expect sat
% @cost { 5 }
% @count optimal 2
%
% The graph splits into two cycles touching the start: a -> b -> a (cost 2) and
% a -> c -> d -> a (cost 3). Each salesman must return to the start, so the two
% cycles split across the two salesmen, for a total of 5, with two co-optimal
% assignments.

#include "../../../encodings/traveling-salesman/variant-02.lp".

vertex(a; b; c; d).

edge(
    a,b,1;
    b,a,1;
    a,c,1;
    c,d,1;
    d,a,1
).

start(a).

salesman(s1; s2).
