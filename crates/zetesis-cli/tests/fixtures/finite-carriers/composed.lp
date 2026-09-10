p(X,Y) :- 0 < X < Y < 3.
{ a }.
n(N) :- N = #count { 1 : a; 2 : a; 3 : a }.
#minimize { 1@N,k : n(N) }.
