% N-Queens (N = 8), first optimization: cardinality row/column constraints.
% @expect sat
% @count  92
%
% The same 92 solutions. Replaces variant-02's pairwise row and column
% inequalities with one cardinality constraint each (exactly one queen per row,
% per column), a smaller and more direct statement of the same requirement. The
% diagonal constraints stay pairwise.

% The board is an n x n grid.
#const n = 8.

row(1..n).
column(1..n).

% queen_at(R,C): a queen stands on row R, column C. Guess any set of placements.
{ queen_at(R,C) : row(R), column(C) }.

% Exactly one queen per row, and exactly one per column.
:- row(R), not 1{ queen_at(R,C) }1.
:- column(C), not 1{ queen_at(R,C) }1.

% No two queens share a diagonal.
:- queen_at(R1,C1), queen_at(R2,C2), (R1,C1) != (R2,C2), R1 - C1 = R2 - C2.
:- queen_at(R1,C1), queen_at(R2,C2), (R1,C1) != (R2,C2), R1 + C1 = R2 + C2.

#show queen_at/2.
