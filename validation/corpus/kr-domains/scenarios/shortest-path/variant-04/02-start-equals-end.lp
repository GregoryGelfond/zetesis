% Scenario 02: Start equals end, no ordering, cap present but slack.
% @expect sat
% @cost { 0 0 }
% @count optimal 1
% @optimal { start(a), end(a) }
%
% The empty edge set has weight 0 and cost 0, satisfying the cap, and no
% before/after facts apply. variant-04 preserves the start = end edge case from
% variant-01.

#include "../../../encodings/shortest-path/variant-04.lp".

vertex(a; b).

edge(
    a,b,5,1;
    b,a,5,1
).

start(a).
end(a).

maximum_cost(10).
