% Exact Vaelii test program: higher-minimize-priority-dominates
% vaelii/vaelii test/vaelii/asp_aspif_test.clj @ 03c728ce8a66
{ a }.
{ b }.
:- a, b.
:- not a, not b.
#minimize { 1@5,1 : a }.
#minimize { 1@1,2 : b }.
