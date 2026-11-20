% Exact Vaelii test program: an-integrity-constraint-forces-the-other-branch
% vaelii/vaelii test/vaelii/asp_aspif_test.clj @ 03c728ce8a66
{ a }.
{ b }.
:- a, b.
:- not a, not b.
#minimize { 1@1,1 : a }.
