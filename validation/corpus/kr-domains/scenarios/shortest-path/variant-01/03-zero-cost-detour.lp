% Scenario 03: Zero-cost detour, separated only by the tie-breaker.
% @expect sat
% @cost { 2 1 }
% @count optimal 1
% @optimal { start(a), included(a,c,2), end(c) }
%
% The direct edge a -> c costs 2. The detour a -> b -> c also costs 2, because
% the b -> c leg is free: routing through b costs nothing extra. Both are
% genuine paths, and the cost objective cannot separate them, so the edge-count
% tie-breaker decides and the one-hop route wins. Without the tie-breaker both
% routes are optimal; this is the case that pins it down.

#include "../../../encodings/shortest-path/variant-01.lp".

vertex(a; b; c).

edge(
    a,c,2;
    a,b,2;
    b,c,0
).

start(a).
end(c).
