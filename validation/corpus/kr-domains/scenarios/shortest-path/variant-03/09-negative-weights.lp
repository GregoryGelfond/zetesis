% Scenario 09: Negative weights, where only the path structure holds the answer.
% @expect sat
% @cost { 2 2 }
% @count optimal 1
% @optimal { start(a), included(a,b,1,1), included(b,c,1,1), end(c) }
%
% The variant-01 negative-weight case on /4 edges. maximum_cost(100) sits well
% clear of any candidate, so the cap is inert and what is on trial is the path
% structure alone. Weights may be negative; resource costs are not. Cost
% minimization wants a negative edge, and only the structure forbids it:
%   b -> d (-5,1)   a spur off an interior vertex, barred by the out-degree bound
%   c -> e (-5,1)   an edge leaving the end, barred by nothing-leaves-the-end
% The answer is a -> b -> c at weight 2. Drop either constraint and the optimum
% becomes -3, on an edge set that is not a start-to-end path.

#include "../../../encodings/shortest-path/variant-03.lp".

vertex(a; b; c; d; e).

edge(
    a,b,1,1;
    b,c,1,1;
    b,d,-5,1;
    c,e,-5,1
).

start(a).
end(c).

maximum_cost(100).
