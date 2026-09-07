% Scenario 07: Graph with cycles.
% @expect sat
% @cost { 3 3 }
% @count optimal 1
% @optimal { start(s), included(d,e,1), included(e,t,1), included(s,d,1), end(t) }
%
% Two s-to-t routes plus back-edges that form cycles among reachable vertices.
% No back-edge can be taken at all: b -> a, c -> b and e -> d would each give a
% vertex a second predecessor, and t -> c would leave the end. So the path
% structure disposes of the cycles, and the cost objective is left to choose
% between the two genuine routes, taking s -> d -> e -> t at weight 3 over the
% four-hop chain at 4.

#include "../../../encodings/shortest-path/variant-01.lp".

vertex(s; a; b; c; d; e; t).

% Forward chain (weight 4 total)
edge(
    s,a,1;
    a,b,1;
    b,c,1;
    c,t,1
).

% Cheaper alternate route (weight 3 total)
edge(
    s,d,1;
    d,e,1;
    e,t,1
).

% Back-edges, each forming a cycle among reachable vertices.
edge(
    b,a,1;
    c,b,1;
    e,d,1;
    t,c,1
).

start(s).
end(t).
