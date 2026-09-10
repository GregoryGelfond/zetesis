{ a }.
n(N) :- N = #count { 1 : a; 2 : a }.
rank(N) :- n(N).
#minimize { 1@P,k : rank(P) }.
