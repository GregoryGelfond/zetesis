% Independently authored admission-regression fixture.
% A cyclic Latin Sudoku with the nine diagonal entries omitted from its givens.
% Each row fixes eight digits, so its last digit determines the unique grid.
% This intentionally tests full 9x9 grounding rather than puzzle difficulty.

axis(1..9).

1 { digit(R,C,V) : axis(V) } 1 :- axis(R), axis(C).
1 { digit(R,C,V) : axis(C) } 1 :- axis(R), axis(V).
1 { digit(R,C,V) : axis(R) } 1 :- axis(C), axis(V).

:- digit(R,C,V), digit(S,D,V), (R-1)/3=(S-1)/3, (C-1)/3=(D-1)/3, R!=S.
:- digit(R,C,V), digit(S,D,V), (R-1)/3=(S-1)/3, (C-1)/3=(D-1)/3, C!=D.

digit(1,2,2). digit(1,3,3). digit(1,4,4). digit(1,5,5). digit(1,6,6). digit(1,7,7). digit(1,8,8). digit(1,9,9).
digit(2,1,4). digit(2,3,6). digit(2,4,7). digit(2,5,8). digit(2,6,9). digit(2,7,1). digit(2,8,2). digit(2,9,3).
digit(3,1,7). digit(3,2,8). digit(3,4,1). digit(3,5,2). digit(3,6,3). digit(3,7,4). digit(3,8,5). digit(3,9,6).
digit(4,1,2). digit(4,2,3). digit(4,3,4). digit(4,5,6). digit(4,6,7). digit(4,7,8). digit(4,8,9). digit(4,9,1).
digit(5,1,5). digit(5,2,6). digit(5,3,7). digit(5,4,8). digit(5,6,1). digit(5,7,2). digit(5,8,3). digit(5,9,4).
digit(6,1,8). digit(6,2,9). digit(6,3,1). digit(6,4,2). digit(6,5,3). digit(6,7,5). digit(6,8,6). digit(6,9,7).
digit(7,1,3). digit(7,2,4). digit(7,3,5). digit(7,4,6). digit(7,5,7). digit(7,6,8). digit(7,8,1). digit(7,9,2).
digit(8,1,6). digit(8,2,7). digit(8,3,8). digit(8,4,9). digit(8,5,1). digit(8,6,2). digit(8,7,3). digit(8,9,5).
digit(9,1,9). digit(9,2,1). digit(9,3,2). digit(9,4,3). digit(9,5,4). digit(9,6,5). digit(9,7,6). digit(9,8,7).

#show digit/3.

