{ a }.
n(N) :- N = #max { 1 : a; word : a }.
m(P) :- P = #count { 1 : a }.
#minimize { N@P,k : n(N), m(P) }.
