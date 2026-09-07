% Scenario 04: Total cost exactly at the budget cap is allowed (boundary).
% @expect sat
% @cost { 2 2 }
% @count optimal 1
% @optimal { start(a), included(a,b,1,5), included(b,c,1,5), end(c) }
%
% Exercises the strict (C > M) cap rather than non-strict (C >= M). The only
% path a -> b -> c has total cost 5 + 5 = 10, exactly the cap. Under :- C > M,
% 10 > 10 is false, so the path is feasible; under :- C >= M it would have been
% excluded as UNSAT.

#include "../../../encodings/shortest-path/variant-03.lp".

vertex(a; b; c).

edge(
    a,b,1,5;
    b,c,1,5
).

start(a).
end(c).

maximum_cost(10).
