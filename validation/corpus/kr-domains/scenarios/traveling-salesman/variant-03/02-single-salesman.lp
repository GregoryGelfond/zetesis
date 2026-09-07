% Scenario 02: Single salesman, single depot, classical TSP.
% @expect sat
% @cost { 3 }
% @count optimal 1
% @optimal { traversed_by(a,b,1,s1), traversed_by(b,c,1,s1), traversed_by(c,a,1,s1), depot(s1,a) }
%
% The variant-01 triangle with one salesman at depot a: classical TSP starting and
% ending at a. The cheap forward cycle (3) beats the expensive reverse (15).

#include "../../../encodings/traveling-salesman/variant-03.lp".

vertex(a; b; c).

edge(
    a,b,1;
    b,c,1;
    c,a,1;
    a,c,5;
    c,b,5;
    b,a,5
).

salesman(s1).
depot(s1,a).
