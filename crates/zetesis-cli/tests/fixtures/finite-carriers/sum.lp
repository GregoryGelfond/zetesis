{ a }.
n(N) :- N = #sum { 2,x : a; 3,y : a }.
#minimize { N@N,k : n(N) }.
