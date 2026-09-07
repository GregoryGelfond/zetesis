% Scenario 04: No path from start to end (UNSAT).
% @expect unsat
%
% Two disconnected components, { a, b } and { c, d }. With start = a and end = d,
% no walk from a to d exists, so no included subset makes d reachable: the
% constraint :- end(Src), not reachable(Src) fires.

#include "../../../encodings/shortest-path/variant-01.lp".

vertex(a; b; c; d).

edge(
    a,b,1;
    c,d,1
).

start(a).
end(d).
