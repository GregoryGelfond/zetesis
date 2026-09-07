% Scenario 04: Ordering forces a path that exceeds the cap (UNSAT).
% @expect unsat
%
% The scenario-03 graph. before(b,t) again forces routing through b, whose only
% ordering-feasible path s -> b -> a -> t has cost 3; maximum_cost(2) rules it
% out. Any path that avoids b violates before(b,t), so no model satisfies both
% layers.

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

maximum_cost(2).
