{ a }.
n(N) :- N = #count { 1 : a; 2 : a }.
#minimize { 1@N,k : n(N) }.
