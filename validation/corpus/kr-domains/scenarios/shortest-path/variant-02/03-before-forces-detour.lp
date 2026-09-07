% Scenario 03: A before/2 constraint forces a more expensive path.
% @expect sat
% @cost { 4 3 }
% @count optimal 1
% @optimal { start(s), included(a,t,1), included(b,a,1), included(s,b,2), end(t) }
%
% Edges: s -> a (1), a -> t (1), s -> b (2), b -> a (1). Unconstrained, the
% optimum is s -> a -> t at cost 2. before(b,t) requires b to reach t over the
% included edges; the only route is b -> a -> t, and the no-dangling-edge
% constraint pulls in s -> b as well. The optimum becomes s -> b -> a -> t at
% cost 4.

#include "../../../encodings/shortest-path/variant-02.lp".

vertex(s; a; b; t).

edge(
    s,a,1;
    a,t,1;
    s,b,2;
    b,a,1
).

start(s).
end(t).

before(b,t).
