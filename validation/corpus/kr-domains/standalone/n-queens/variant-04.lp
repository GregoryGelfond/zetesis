% N-Queens (N = 8), second optimization: diagonals reified by enumeration.
% @expect sat
% @count  92
%
% The same 92 solutions. Replaces variant-03's pairwise diagonal constraints
% with one cardinality constraint per diagonal, indexing the 2n-1 diagonals of
% each direction by D = R - C + n and D = R + C - 1. Diagonal membership is
% computed inline in each constraint.

% The board is an n x n grid.
#const n = 8.

row(1..n).
column(1..n).

% queen_at(R,C): a queen stands on row R, column C. Guess any set of placements.
{ queen_at(R,C) : row(R), column(C) }.

% Exactly one queen per row, and exactly one per column.
:- row(R), not 1{ queen_at(R,C) }1.
:- column(C), not 1{ queen_at(R,C) }1.

% At most one queen on each diagonal: the 2n-1 diagonals of each direction
% indexed by D = R - C + n and D = R + C - 1.
:- D = 1..2*n-1, not { queen_at(R,C) : D = R-C+n }1.
:- D = 1..2*n-1, not { queen_at(R,C) : D = R+C-1 }1.

#show queen_at/2.
