% Enumerate all subsets of distinct positive weights 1..n whose sum is n.
% Each item has its own identity and a weight equal to that identity. Including
% the item in the aggregate tuple makes the contribution identity explicit.

#const n = 12.

item(1..n).
{ selected(I) : item(I) }.
:- #sum { I, I : selected(I) } != n.

#show selected/1.
