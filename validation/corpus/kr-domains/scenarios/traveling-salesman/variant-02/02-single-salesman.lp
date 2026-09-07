% Scenario 02: Single salesman degenerates to classical TSP.
% @expect sat
% @cost { 3 }
% @count optimal 1
% @optimal { traversed_by(a,b,1,s1), traversed_by(b,c,1,s1), traversed_by(c,a,1,s1), start(a) }
%
% With one salesman, variant-02 reduces to variant-01: a single Hamiltonian tour.
% The variant-01 triangle, cheap forward cycle (3) against expensive reverse (15).

#include "../../../encodings/traveling-salesman/variant-02.lp".

vertex(a; b; c).

edge(
    a,b,1;
    b,c,1;
    c,a,1;
    a,c,5;
    c,b,5;
    b,a,5
).

start(a).

salesman(s1).
