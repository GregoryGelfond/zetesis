d(1;foo).
p(X) :- d(X), X+1>0.
#minimize{1:p(X)}.
