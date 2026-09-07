% N-Queens (N = 8), naive portable encoding.
% @expect sat
% @count  92
%
% Places exactly one queen on each row and each column, and forbids two queens
% on a shared diagonal. These constraints admit exactly the 92 solutions of the
% 8-queens problem. Uses only portable ASP-Core-2 constructs, so it runs on
% clingo and I-DLV alike.

% The board is an 8 x 8 grid.
row(1..8).
column(1..8).

% queen_at(R,C): a queen stands on row R, column C.
% Exactly one queen per row, and exactly one per column.
1 <= { queen_at(R,C) : column(C) } <= 1 :- row(R).
1 <= { queen_at(R,C) : row(R) } <= 1 :- column(C).

% No two queens share a diagonal: the "\" direction (constant R - C), then "/"
% (constant R + C).
:- queen_at(R1,C1), queen_at(R2,C2), R1 < R2, R1 - C1 = R2 - C2.
:- queen_at(R1,C1), queen_at(R2,C2), R1 < R2, R1 + C1 = R2 + C2.

#show queen_at/2.
