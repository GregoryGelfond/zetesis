% Scenario 01: Basic 3-vertex TSP with a cheap and an expensive cycle.
% @expect sat
% @cost { 3 }
% @count optimal 1
% @optimal { included(a,b,1), included(b,c,1), included(c,a,1), start(a) }
%
% Two Hamiltonian cycles through a: the forward a -> b -> c -> a at unit cost (3)
% and the reverse a -> c -> b -> a at cost 5 each (15). The optimum is the cheap
% one.

#include "../../../encodings/traveling-salesman/variant-01.lp".

vertex(a; b; c).

% Cheap cycle direction.
edge(
    a,b,1;
    b,c,1;
    c,a,1
).

% Expensive reverse direction.
edge(
    a,c,5;
    c,b,5;
    b,a,5
).

start(a).
