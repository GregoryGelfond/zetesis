% Scenario 02: Start equals end, no ordering constraints.
% @expect sat
% @cost { 0 0 }
% @count optimal 1
% @optimal { start(a), end(a) }
%
% The start = end edge case still resolves to the empty path in variant-02: the
% reachable(V) :- start(V) base case makes the start reachable, and no
% before/after facts apply.

#include "../../../encodings/shortest-path/variant-02.lp".

vertex(a; b).

edge(
    a,b,5;
    b,a,5
).

start(a).
end(a).
