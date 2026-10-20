% Select distinct items whose weights n+1..2*n sum to target.
% The item identity remains in the sum tuple, independently of its weight.

#const n = 8.
#const target = 17.

item(1..n).
weight(I,n+I) :- item(I).
{ selected(I) : item(I) }.
:- #sum { W,I : selected(I), weight(I,W) } != target.

#show selected/1.
