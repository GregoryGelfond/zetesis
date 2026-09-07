% Scenario 05: Ordering constraint that cannot be satisfied (UNSAT).
% @expect unsat
%
% A single linear path a -> b (1), b -> c (1). Reaching end = c requires both
% edges, which makes b reachable. Then before(c,b) requires c to reach b, but c
% has no outgoing edges, so :- before(X,Y), reachable(Y), not reachable_from(X,Y)
% fires. The program is UNSAT.

#include "../../../encodings/shortest-path/variant-02.lp".

vertex(a; b; c).

edge(
    a,b,1;
    b,c,1
).

start(a).
end(c).

before(c,b).
