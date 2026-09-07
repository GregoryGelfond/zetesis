% Scenario 02: Surveillance bound satisfied exactly at the limit.
% @expect sat
% @cost { 3 }
% @cautious optimal { traversed_by(a,b,1,s1), traversed_by(b,c,1,s1), traversed_by(c,a,1,s1), depot(s1,a) }
%
% A single salesman patrolling a -> b -> c -> a, each edge cost 1 and time 2, so
% tour_time(s1) = 6. max_revisit_interval(b,6) and (c,6) hold at the bound: the
% encoded inequality is tour_time <= 6, so equality qualifies.

#include "../../../encodings/traveling-salesman/variant-05.lp".

vertex(a; b; c).

edge(
    a,b,1;
    b,c,1;
    c,a,1
).

edge_time(
    a,b,2;
    b,c,2;
    c,a,2
).

salesman(s1).
depot(s1,a).

max_revisit_interval(
    b,6;
    c,6
).
