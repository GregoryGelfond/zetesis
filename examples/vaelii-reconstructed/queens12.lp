#const n=12.
1 { q(X,1..n) } 1 :- X=1..n.
:- q(X1,Y), q(X2,Y), X1 < X2.
:- q(X1,Y1), q(X2,Y2), X1 < X2, X2-X1 = |Y2-Y1|.
#show q/2.
