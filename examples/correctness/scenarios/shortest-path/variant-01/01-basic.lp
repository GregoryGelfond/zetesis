% Scenario 01: Basic shortest path on a triangle.
%
% The direct edge a -> c costs 3; the two-hop path a -> b -> c costs 2, so the
% two-hop path is shortest.

#include "../../../encodings/shortest-path/variant-01.lp".

vertex(a; b; c).

edge(
    a,b,1;
    b,c,1;
    a,c,3
).

start(a).
end(c).
