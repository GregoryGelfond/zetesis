{p(1,2);p(2,4)}.
q :- p(X,X+1):#true.
b. {a}.
n(N) :- N=#max{2:a;foo:b}.
v(X) :- n(X).
#false | marker.
#minimize{X@7:v(X);bar@5;0@3}.
