% Exact Vaelii test program: a-weight-constraint-bounds-a-cardinality
% vaelii/vaelii test/vaelii/asp_aspif_test.clj @ 03c728ce8a66
{ a }.
{ b }.
{ c }.
:- 2 #sum { 1,1 : a; 1,2 : b; 1,3 : c }.
:- 3 #sum { 1,1 : not a; 1,2 : not b; 1,3 : not c }.
#minimize { 1@1,1 : a; 1@1,2 : b }.
