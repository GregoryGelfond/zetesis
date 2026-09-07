% Scenario 01: Two salesmen, two depots, one customer each.
%
% Two disjoint 2-cycles, one per depot-customer pair: s1 at depot a serves c
% (a -> c -> a, cost 2), s2 at depot b serves d (b -> d -> b, cost 2). No
% cross-cluster edges, so each salesman is confined to its own pair. Total 4.

#include "../../../encodings/traveling-salesman/variant-03.lp".

vertex(a; b; c; d).

edge(
    a,c,1;
    c,a,1;
    b,d,1;
    d,b,1
).

salesman(s1; s2).
depot(
    s1,a;
    s2,b
).
