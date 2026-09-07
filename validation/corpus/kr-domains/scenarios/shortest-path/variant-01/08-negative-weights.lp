% Scenario 08: Negative weights, where only the path structure holds the answer.
% @expect sat
% @cost { 2 2 }
% @count optimal 1
% @optimal { start(a), included(a,b,1), included(b,c,1), end(c) }
%
% Edge weights may be negative, and this is where the path structure earns its
% place: cost minimization actively wants to take a negative edge, and only the
% structure forbids it. Two temptations are on offer:
%   b -> d (-5)   a spur off an interior vertex, barred by the out-degree bound
%   c -> e (-5)   an edge leaving the end, barred by nothing-leaves-the-end
% The answer is the honest path a -> b -> c at weight 2. Drop either constraint
% and the optimum becomes -3, on an edge set that is not a start-to-end path.
% This case is also why the encoding carries no non-negativity assumption: the
% structure, not the sign of the weights, is what makes the answer a path.

#include "../../../encodings/shortest-path/variant-01.lp".

vertex(a; b; c; d; e).

edge(
    a,b,1;
    b,c,1;
    b,d,-5;
    c,e,-5
).

start(a).
end(c).
