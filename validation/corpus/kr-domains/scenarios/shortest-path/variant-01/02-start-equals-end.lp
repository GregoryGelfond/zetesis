% Scenario 02: Start equals end.
% @expect sat
% @cost { 0 0 }
% @count optimal 1
% @optimal { start(a), end(a) }
%
% When start and end coincide, the empty edge set (cost 0) is optimal. Exercises
% the reachable(V) :- start(V) base case: the start reaches itself, so no edge is
% needed for end-reachability.

#include "../../../encodings/shortest-path/variant-01.lp".

vertex(a; b).

edge(
    a,b,5;
    b,a,5
).

start(a).
end(a).
