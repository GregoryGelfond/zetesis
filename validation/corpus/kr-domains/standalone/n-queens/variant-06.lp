% N-Queens (N = 8), fourth optimization: bounded generation.
% @expect sat
% @count  92
%
% The same 92 solutions. Folds the row/column requirement into generation: each
% row, and each column, directly places exactly one queen via a bounded choice
% rule, so the separate "exactly one per row/column" constraints of variant-05
% are no longer needed. Diagonals stay memoized as in variant-05.

% The board is an n x n grid.
#const n = 8.

row(1..n).
column(1..n).

% queen_at(R,C): a queen stands on row R, column C.
% Place exactly one queen on each row, and exactly one on each column.
1{ queen_at(R,C) : column(C) }1 :- row(R).
1{ queen_at(R,C) : row(R) }1 :- column(C).

% At most one queen on each diagonal of either direction.
:- D = 1..2*n-1, not { queen_at(R,C) : diagonal_type_1(R,C,D) }1.
:- D = 1..2*n-1, not { queen_at(R,C) : diagonal_type_2(R,C,D) }1.

% diagonal_type_1(R,C,D) / diagonal_type_2(R,C,D): cell (R,C) lies on diagonal D
% of the "\" direction (D = R - C + n) and the "/" direction (D = R + C - 1).
diagonal_type_1(R,C,R-C+n) :- row(R), column(C).
diagonal_type_2(R,C,R+C-1) :- row(R), column(C).

#show queen_at/2.
