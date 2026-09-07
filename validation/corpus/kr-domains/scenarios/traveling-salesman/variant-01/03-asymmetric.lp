% Scenario 03: Asymmetric 4-vertex graph, traversal direction matters.
% @expect sat
% @cost { 4 }
% @count optimal 1
% @optimal { included(a,b,1), included(b,c,1), included(c,d,1), included(d,a,1), start(a) }
%
% The square's edges are cheap clockwise (cost 1) and expensive counter-clockwise
% (cost 10), with symmetric diagonals at cost 5. The clockwise tour
% a -> b -> c -> d -> a costs 4; the reverse costs 40, and any mixed tour takes an
% expensive edge or a diagonal and costs more.

#include "../../../encodings/traveling-salesman/variant-01.lp".

vertex(a; b; c; d).

% Cheap clockwise direction.
edge(
    a,b,1;
    b,c,1;
    c,d,1;
    d,a,1
).

% Expensive counter-clockwise direction.
edge(
    b,a,10;
    c,b,10;
    d,c,10;
    a,d,10
).

% Diagonals (symmetric).
edge(
    a,c,5;
    c,a,5;
    b,d,5;
    d,b,5
).

start(a).
