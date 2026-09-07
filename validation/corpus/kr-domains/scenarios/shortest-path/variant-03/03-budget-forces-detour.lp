% Scenario 03: Budget cap forces a higher-weight path.
% @expect sat
% @cost { 4 2 }
% @count optimal 1
% @optimal { start(s), included(a,t,2,1), included(s,a,2,1), end(t) }
%
% Two routes run from s to t:
%   s -> t       weight 1, cost 10  (least weight, but over budget)
%   s -> a -> t  weight 4, cost 2
% maximum_cost(5) rules out the direct edge (10 > 5), so the detour through a
% is the only feasible route, at weight 4.

#include "../../../encodings/shortest-path/variant-03.lp".

vertex(s; a; t).

edge(
    s,t,1,10;
    s,a,2,1;
    a,t,2,1
).

start(s).
end(t).

maximum_cost(5).
