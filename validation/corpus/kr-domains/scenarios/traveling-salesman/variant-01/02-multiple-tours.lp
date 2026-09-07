% Scenario 02: Fully-connected 4-vertex graph, symmetric costs.
% @expect sat
% @cost { 8 }
% @count optimal 2
%
% Six undirected edges (each as two directed edges of equal cost): a-b 1, a-c 3,
% a-d 4, b-c 2, b-d 5, c-d 1. The three Hamiltonian cycles cost:
%   a-b-c-d-a  1 + 2 + 1 + 4 = 8   (optimum)
%   a-b-d-c-a  1 + 5 + 1 + 3 = 10
%   a-c-b-d-a  3 + 2 + 5 + 4 = 14
% The optimum cycle is co-optimal in its two traversal directions.

#include "../../../encodings/traveling-salesman/variant-01.lp".

vertex(a; b; c; d).

edge(
    a,b,1;
    b,a,1;
    a,c,3;
    c,a,3;
    a,d,4;
    d,a,4;
    b,c,2;
    c,b,2;
    b,d,5;
    d,b,5;
    c,d,1;
    d,c,1
).

start(a).
