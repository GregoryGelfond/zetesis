% N-Queens (N = 8), third optimization: memoized diagonal indices.
% @expect sat
% @count  92
%
% The same 92 solutions. Lifts variant-04's inline diagonal arithmetic into two
% predicates, diagonal_type_1/2, computed once over the board and reused in the
% diagonal constraints rather than recomputed in place.

% The board is an n x n grid.
#const n = 8.

row(1..n).
column(1..n).

% queen_at(R,C): a queen stands on row R, column C. Guess any set of placements.
{ queen_at(R,C) : row(R), column(C) }.

% Exactly one queen per row, and exactly one per column.
:- row(R), not 1{ queen_at(R,C) }1.
:- column(C), not 1{ queen_at(R,C) }1.

% At most one queen on each diagonal of either direction.
:- D = 1..2*n-1, not { queen_at(R,C) : diagonal_type_1(R,C,D) }1.
:- D = 1..2*n-1, not { queen_at(R,C) : diagonal_type_2(R,C,D) }1.

% diagonal_type_1(R,C,D) / diagonal_type_2(R,C,D): cell (R,C) lies on diagonal D
% of the "\" direction (D = R - C + n) and the "/" direction (D = R + C - 1).
diagonal_type_1(R,C,R-C+n) :- row(R), column(C).
diagonal_type_2(R,C,R+C-1) :- row(R), column(C).

#show queen_at/2.
