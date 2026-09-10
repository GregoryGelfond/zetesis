{ a }.
n(N) :- N = #count { 1 : a }.
m(P) :- P = #sum { 2 : a }.
#minimize { 1@(N+P),k : n(N), m(P) }.
