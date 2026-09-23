% N-Queens, parametric. Place n mutually non-attacking queens on an n x n board.
%
% One scaling knob, #const n, spans a wide enumeration workload: the answer-set
% count grows steeply with n (n=8 -> 92, n=12 -> 14200, n=13 -> 73712), so the
% same encoding measures enumeration throughput across board sizes. Uses only
% portable ASP-Core-2 constructs, so clingo and zetesis run it unchanged.

#const n = 8.

row(1..n).
col(1..n).

% Exactly one queen on each row, and exactly one on each column.
1 { queen(R,C) : col(C) } 1 :- row(R).
1 { queen(R,C) : row(R) } 1 :- col(C).

% No two queens share a diagonal: "\" (constant R - C) then "/" (constant R + C).
:- queen(R1,C1), queen(R2,C2), R1 < R2, R1 - C1 = R2 - C2.
:- queen(R1,C1), queen(R2,C2), R1 < R2, R1 + C1 = R2 + C2.

#show queen/2.
