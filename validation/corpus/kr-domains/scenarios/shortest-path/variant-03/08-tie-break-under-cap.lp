% Scenario 08: The cap leaves two equal-weight routes; the tie-breaker decides.
% @expect sat
% @cost { 3 2 }
% @count optimal 1
% @optimal { start(s), included(s,b,2,1), included(b,t,1,1), end(t) }
%
% Edges (weight, cost): s -> t (1,10), s -> b (2,1), b -> t (1,1), s -> a (1,1),
% a -> b (1,1). The direct s -> t is the lightest route at weight 1, but its cost
% of 10 exceeds maximum_cost(5). Two routes remain and both weigh 3: s -> b -> t
% at two hops (cost 2) and s -> a -> b -> t at three (cost 3). Both fit the
% budget, so the cap cannot separate them either, and the edge-count tie-breaker
% decides. This is where variant-03's tie-breaker is pinned down: without it,
% both routes are optimal.

#include "../../../encodings/shortest-path/variant-03.lp".

vertex(s; a; b; t).

edge(
    s,t,1,10;
    s,b,2,1;
    b,t,1,1;
    s,a,1,1;
    a,b,1,1
).

start(s).
end(t).

maximum_cost(5).
