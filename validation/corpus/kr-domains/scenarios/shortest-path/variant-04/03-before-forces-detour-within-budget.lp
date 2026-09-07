% Scenario 03: before/2 forces a longer path; the budget accommodates it.
% @expect sat
% @cost { 4 3 }
% @count optimal 1
% @optimal { start(s), included(a,t,1,1), included(b,a,1,1), included(s,b,2,1), end(t) }
%
% Edges (weight, cost): s -> a (1,1), s -> b (2,1), a -> t (1,1), b -> a (1,1).
% Unconstrained, the minimum-weight path is s -> a -> t (weight 2). before(b,t)
% forces b onto the path; the cheapest route satisfying both end-reachability and
% the ordering is s -> b -> a -> t, weight 4, cost 3, within maximum_cost(100).

#include "../../../encodings/shortest-path/variant-04.lp".

vertex(s; a; b; t).

edge(
    s,a,1,1;
    a,t,1,1;
    s,b,2,1;
    b,a,1,1
).

start(s).
end(t).

before(b,t).

maximum_cost(100).
