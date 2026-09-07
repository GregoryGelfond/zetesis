% Scenario 06: Layered DAG.
% @expect sat
% @cost { 4 4 }
% @count optimal 1
% @optimal { start(s), included(a1,b1,1), included(b1,c1,1), included(c1,t,1), included(s,a1,1), end(t) }
%
% 11 vertices in a 4-layer DAG, s -> {a1,a2,a3} -> {b1,b2,b3} -> {c1,c2,c3} -> t,
% with 24 edges and 27 distinct s-to-t walks. Weights are set so the locally
% cheapest hop at each layer is also globally shortest: s -> a1 -> b1 -> c1 -> t
% at weight 4.

#include "../../../encodings/shortest-path/variant-01.lp".

vertex(s; a1; a2; a3; b1; b2; b3; c1; c2; c3; t).

% Source -> Layer A
edge(
    s,a1,1;
    s,a2,4;
    s,a3,2
).

% Layer A -> Layer B
edge(
    a1,b1,1;
    a1,b2,3;
    a1,b3,5;
    a2,b1,2;
    a2,b2,1;
    a2,b3,3;
    a3,b1,4;
    a3,b2,2;
    a3,b3,1
).

% Layer B -> Layer C
edge(
    b1,c1,1;
    b1,c2,3;
    b1,c3,5;
    b2,c1,2;
    b2,c2,1;
    b2,c3,3;
    b3,c1,4;
    b3,c2,2;
    b3,c3,1
).

% Layer C -> Sink
edge(
    c1,t,1;
    c2,t,3;
    c3,t,2
).

start(s).
end(t).
