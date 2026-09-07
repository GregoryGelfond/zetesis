% SEND + MORE = MONEY.
% @expect sat
% @count  1
% @model  { assign(s,9), assign(e,5), assign(n,6), assign(d,7), assign(m,1), assign(o,0), assign(r,8), assign(y,2) }
%
% A cryptarithmetic puzzle: assign the eight letters {S, E, N, D, M, O, R, Y} to
% distinct decimal digits so that SEND + MORE = MONEY holds when each word is read
% as a multi-digit number. S and M are leading digits, so they are nonzero. The
% solution is unique: S=9, E=5, N=6, D=7, M=1, O=0, R=8, Y=2 (9567 + 1085 = 10652).

% Domain encoding

% letter(L): L is one of the puzzle's letters.
letter(s; e; n; d; m; o; r; y).

% digit(D): D is a decimal digit.
digit(0..9).

% assign(L,D): letter L is assigned digit D. Exactly one digit per letter, all
% distinct, with nonzero leading digits.
1 { assign(L,D) : digit(D) } 1 :- letter(L).

:- assign(L1,D), assign(L2,D), L1 < L2.
:- assign(s,0).
:- assign(m,0).

% carry_at(I,V): the carry into column I is V (0 or 1). The identity is decomposed
% column by column with explicit carries; without that, a single rule over all
% eight letters would ground to 10^8 instances. Each column rule has at most three
% letters plus two carries, keeping the per-rule grounding small.
carry_index(1..4).
carry_value(0; 1).
1 { carry_at(I,V) : carry_value(V) } 1 :- carry_index(I).

% Column 0 (units): D + E = Y + 10*C1
:- assign(d,D),
   assign(e,E),
   assign(y,Y),
   carry_at(1,C1),
   D + E != Y + 10*C1.

% Column 1 (tens): N + R + C1 = E + 10*C2
:- assign(n,N),
   assign(r,R),
   assign(e,E),
   carry_at(1,C1),
   carry_at(2,C2),
   N + R + C1 != E + 10*C2.

% Column 2 (hundreds): E + O + C2 = N + 10*C3
:- assign(e,E),
   assign(o,O),
   assign(n,N),
   carry_at(2,C2),
   carry_at(3,C3),
   E + O + C2 != N + 10*C3.

% Column 3 (thousands): S + M + C3 = O + 10*C4
:- assign(s,S),
   assign(m,M),
   assign(o,O),
   carry_at(3,C3),
   carry_at(4,C4),
   S + M + C3 != O + 10*C4.

% Column 4 (ten-thousands): C4 = M
% The final carry is MONEY's leading digit; with M nonzero this forces M = C4 = 1.
:- assign(m,M), carry_at(4,C4), C4 != M.

% Solver directives

#show assign/2.
