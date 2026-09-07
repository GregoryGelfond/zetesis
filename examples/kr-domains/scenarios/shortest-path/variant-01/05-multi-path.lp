% Scenario 05: Three competing routes.
%
% Three routes from s to t:
%   s -> t            cost 10
%   s -> a -> t       cost 4 + 4 = 8
%   s -> b -> c -> t  cost 1 + 1 + 1 = 3   (cheapest)
% Cost minimization takes the longest-hop route because it is cheapest; the
% edge-count tie-breaker does not override the cost objective.

#include "../../../encodings/shortest-path/variant-01.lp".

vertex(s; a; b; c; t).

edge(
    s,t,10;
    s,a,4;
    a,t,4;
    s,b,1;
    b,c,1;
    c,t,1
).

start(s).
end(t).
