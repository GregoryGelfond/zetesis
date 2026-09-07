% Scenario 08: Ordering leaves two equal-cost routes; the tie-breaker decides.
% @expect sat
% @cost { 3 2 }
% @count optimal 1
% @optimal { start(s), included(s,b,2), included(b,t,1), end(t) }
%
% Edges: s -> t (1), s -> b (2), b -> t (1), s -> a (1), a -> b (1). The direct
% s -> t is the cheapest route at cost 1, but it skips b, so before(b,t) rules it
% out. Two routes through b remain and both cost 3: s -> b -> t at two hops and
% s -> a -> b -> t at three. The cost objective cannot separate them, so the
% edge-count tie-breaker decides and the two-hop route wins. This is where
% variant-02's tie-breaker is pinned down: without it, both routes are optimal.

#include "../../../encodings/shortest-path/variant-02.lp".

vertex(s; a; b; t).

edge(
    s,t,1;
    s,b,2;
    b,t,1;
    s,a,1;
    a,b,1
).

start(s).
end(t).

before(b,t).
