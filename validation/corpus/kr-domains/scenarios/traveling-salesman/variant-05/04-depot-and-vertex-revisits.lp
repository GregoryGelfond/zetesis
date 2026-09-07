% Scenario 04: Several revisit intervals, including one on the depot.
% @expect sat
% @cost { 3 }
% @cautious optimal { traversed_by(a,b,1,s1), traversed_by(b,c,1,s1), traversed_by(c,a,1,s1), depot(s1,a) }
%
% The 3-cycle a -> b -> c -> a, tour_time 6, with a also s1's depot.
% max_revisit_interval(a,8) bounds tour_time via the depot (visited_by(a,s1) holds
% through the closing arc), and (b,7) and (c,8) bound it through the same rule on
% non-depot vertices. All hold: 6 <= 8, 6 <= 7, 6 <= 8. Exercises several
% surveillance bounds on one tour, one of them on the depot.

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
    a,8;
    b,7;
    c,8
).
