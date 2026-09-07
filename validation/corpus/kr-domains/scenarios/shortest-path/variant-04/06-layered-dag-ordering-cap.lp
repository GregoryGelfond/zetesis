% Scenario 06: Layered DAG with an ordering constraint and a cost cap.
% @expect sat
% @cost { 8 4 }
% @count optimal 2
% @cautious optimal { included(a3,b3,1,5), included(s,a3,2,3) }
%
% The variant-03 layered DAG, with both layers active: before(b3,t) forces the
% path through b3, and maximum_cost(15) bounds the resource cost, restricting
% which b3-routes are feasible. The two optima share the s -> a3 -> b3 entry.

#include "../../../encodings/shortest-path/variant-04.lp".

vertex(s; a1; a2; a3; b1; b2; b3; c1; c2; c3; t).

edge(
    s,a1,1,5;
    s,a2,4,1;
    s,a3,2,3;
    a1,b1,1,5;
    a1,b2,3,2;
    a1,b3,5,1;
    a2,b1,2,3;
    a2,b2,1,5;
    a2,b3,3,2;
    a3,b1,4,1;
    a3,b2,2,3;
    a3,b3,1,5;
    b1,c1,1,5;
    b1,c2,3,2;
    b1,c3,5,1;
    b2,c1,2,3;
    b2,c2,1,5;
    b2,c3,3,2;
    b3,c1,4,1;
    b3,c2,2,3;
    b3,c3,1,5;
    c1,t,1,5;
    c2,t,3,2;
    c3,t,2,3
).

start(s).
end(t).

before(b3,t).

maximum_cost(15).
