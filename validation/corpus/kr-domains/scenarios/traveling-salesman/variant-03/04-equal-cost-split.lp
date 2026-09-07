% Scenario 04: Two depots, two customers, uniform cost.
% @expect sat
% @cost { 4 }
% @count optimal 2
%
% Each depot has a cost-1 edge (both directions) to each customer: a <-> c,
% a <-> d, b <-> c, b <-> d. Either customer-to-salesman assignment costs the same
% (s1:c + s2:d, or s1:d + s2:c, each 4), so two co-optimal models exist.

#include "../../../encodings/traveling-salesman/variant-03.lp".

vertex(a; b; c; d).

edge(
    a,c,1;
    c,a,1;
    a,d,1;
    d,a,1;
    b,c,1;
    c,b,1;
    b,d,1;
    d,b,1
).

salesman(s1; s2).
depot(
    s1,a;
    s2,b
).
