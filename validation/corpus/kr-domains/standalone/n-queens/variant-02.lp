% N-Queens (N = 8), guess-and-check encoding.
% @expect sat
% @count  92
%
% The same 92 solutions as variant-01, reached differently: guess an arbitrary
% placement, then rule out any board that does not hold exactly n queens, or
% that puts two queens in one row, one column, or one diagonal. Relies on
% clingo-specific constructs (tuple inequality, the n{...}n bound), so unlike
% variant-01 it is clingo-only.

% The board is an n x n grid.
#const n = 8.

row(1..n).
column(1..n).

% queen_at(R,C): a queen stands on row R, column C. Guess any set of placements.
{ queen_at(R,C) : row(R), column(C) }.

% Exactly n queens are placed.
:- not n{ queen_at(R,C) }n.

% No two queens share a row, a column, or a diagonal.
:- queen_at(R1,C1), queen_at(R1,C2), C1 != C2.
:- queen_at(R1,C1), queen_at(R2,C1), R1 != R2.
:- queen_at(R1,C1), queen_at(R2,C2), (R1,C1) != (R2,C2), R1 - C1 = R2 - C2.
:- queen_at(R1,C1), queen_at(R2,C2), (R1,C1) != (R2,C2), R1 + C1 = R2 + C2.

#show queen_at/2.
