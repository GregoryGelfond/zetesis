% Scenario 02: Start equals end, with a budget cap present.
% @expect sat
% @cost { 0 0 }
% @count optimal 1
% @optimal { start(a), end(a) }
%
% The empty edge set has weight 0 and cost 0, satisfying any non-negative cap.
% Confirms the reachable(V) :- start(V) base case carries over to variant-03,
% and that the cap is a no-op at cost 0.

#include "../../../encodings/shortest-path/variant-03.lp".

vertex(a; b).

edge(
    a,b,5,1;
    b,a,5,1
).

start(a).
end(a).

maximum_cost(10).
