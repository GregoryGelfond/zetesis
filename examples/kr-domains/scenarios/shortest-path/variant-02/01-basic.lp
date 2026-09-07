% Scenario 01: Basic shortest path, no ordering constraints.
%
% The variant-01 triangle. With no before/after facts, variant-02 reduces to
% variant-01: the direct edge a -> c (cost 3) loses to a -> b -> c (cost 2).

#include "../../../encodings/shortest-path/variant-02.lp".

vertex(a; b; c).

edge(
    a,b,1;
    b,c,1;
    a,c,3
).

start(a).
end(c).
