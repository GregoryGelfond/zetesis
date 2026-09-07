% Scenario 01: Basic shortest path, no ordering, loose budget.
% @expect sat
% @cost { 2 2 }
% @count optimal 1
% @optimal { start(a), included(a,b,1,0), included(b,c,1,0), end(c) }
%
% The triangle on /4 edges (weight, resource cost). With no before/after facts
% and the cap well above any feasible cost, variant-04 reduces to variant-01:
% the two-hop path a -> b -> c (weight 2) beats the direct a -> c (weight 3).

#include "../../../encodings/shortest-path/variant-04.lp".

vertex(a; b; c).

edge(
    a,b,1,0;
    b,c,1,0;
    a,c,3,0
).

start(a).
end(c).

maximum_cost(100).
