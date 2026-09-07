% Scenario 06: Layered DAG with a before/2 constraint.
% @expect sat
% @cost { 7 4 }
% @count optimal 2
% @cautious optimal { included(b2,c1,2), included(c1,t,1) }
%
% The variant-01 layered DAG. Unconstrained, the shortest path is
% s -> a1 -> b1 -> c1 -> t (weight 4). before(b2,t) requires the path to pass
% through b2; the cheapest routes that do tie at weight 7 (for instance
% s -> a1 -> b2 -> c1 -> t and s -> a3 -> b2 -> c1 -> t). Both optima share the
% b2 -> c1 -> t tail.

#include "../../../encodings/shortest-path/variant-02.lp".

vertex(s; a1; a2; a3; b1; b2; b3; c1; c2; c3; t).

edge(
    s,a1,1;
    s,a2,4;
    s,a3,2;
    a1,b1,1;
    a1,b2,3;
    a1,b3,5;
    a2,b1,2;
    a2,b2,1;
    a2,b3,3;
    a3,b1,4;
    a3,b2,2;
    a3,b3,1;
    b1,c1,1;
    b1,c2,3;
    b1,c3,5;
    b2,c1,2;
    b2,c2,1;
    b2,c3,3;
    b3,c1,4;
    b3,c2,2;
    b3,c3,1;
    c1,t,1;
    c2,t,3;
    c3,t,2
).

start(s).
end(t).

before(b2,t).
