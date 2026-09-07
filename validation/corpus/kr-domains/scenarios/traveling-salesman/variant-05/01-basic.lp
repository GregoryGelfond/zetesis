% Scenario 01: Two depots, one customer each, with loose revisit intervals.
% @expect sat
% @cost { 4 }
% @cautious optimal { traversed_by(a,c,1,s1), traversed_by(b,d,1,s2), traversed_by(c,a,1,s1), traversed_by(d,b,1,s2), depot(s1,a), depot(s2,b) }
%
% The variant-04 two-cluster shape plus the variant-05 surveillance layer: s1 runs
% a -> c -> a, s2 runs b -> d -> b (each cost 2, tour_time 2).
% max_revisit_interval(c,10) and (d,10) sit well above the tour_time of 2, so the
% surveillance bound holds without changing the optimum. Total 4.

#include "../../../encodings/traveling-salesman/variant-05.lp".

vertex(a; b; c; d).

edge(
    a,c,1;
    c,a,1;
    b,d,1;
    d,b,1
).

edge_time(
    a,c,1;
    c,a,1;
    b,d,1;
    d,b,1
).

salesman(s1; s2).
depot(
    s1,a;
    s2,b
).

max_revisit_interval(
    c,10;
    d,10
).
